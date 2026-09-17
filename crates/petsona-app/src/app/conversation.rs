use super::geometry::{ease_out, entry_progress, lerp, smoothstep};
use super::*;

const CONVERSATION_TITLE: &str = "Petsona 对话";
pub(super) const CONVERSATION_PILL_WIDTH: f32 = 420.0;
pub(super) const CONVERSATION_PILL_HEIGHT: f32 = 56.0;
const CONVERSATION_OVERLAY_PADDING: f32 = 12.0;
pub(super) const CONVERSATION_GAP: f32 = 10.0;
/// The conversation window grows out of the shadow under the pet and collapses
/// back into it when it closes.
pub(super) const CONVERSATION_ENTRY: Duration = Duration::from_millis(240);
pub(super) const CONVERSATION_EXIT: Duration = Duration::from_millis(200);
const CONVERSATION_COLLAPSED_SCALE: f32 = 0.06;
const CONVERSATION_SHADOW_Y_OFFSET: f32 = 3.0;

impl PetsonaApp {
    pub(super) fn open_conversation(&mut self) {
        if !self.conversation_open {
            self.conversation_shown_at = Some(Instant::now());
        }
        self.conversation_open = true;
        self.conversation_closing_at = None;
        self.conversation_focus_pending = true;
        self.conversation_input_focus_pending = true;
    }

    pub(super) fn close_conversation(&mut self) {
        if !self.conversation_open && self.conversation_closing_at.is_none() {
            return;
        }
        self.conversation_open = false;
        self.conversation_focus_pending = false;
        self.conversation_input_focus_pending = false;
        self.conversation_cursor = None;
        if self.conversation_window_created {
            self.conversation_closing_at = Some(Instant::now());
        } else {
            self.conversation_closing_at = None;
            self.conversation_shown_at = None;
        }
    }

    fn conversation_history_text(&self) -> String {
        self.conversation_history
            .iter()
            .rev()
            .take(6)
            .rev()
            .map(|turn| {
                if turn.user {
                    format!("用户：{}", turn.text)
                } else {
                    format!("宠物：{}", turn.text)
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn remember_user_preferences(&self, text: &str) {
        let trimmed = text.trim();
        let preference = [
            "我喜欢",
            "我喜歡",
            "我偏好",
            "我习惯",
            "我習慣",
            "我不喜欢",
            "我不喜歡",
        ]
        .iter()
        .any(|prefix| trimmed.starts_with(prefix));
        if preference && trimmed.chars().count() >= 4 {
            let _ = self
                .memory
                .remember_fact(&self.persona.id, "用户偏好", trimmed, 0.75);
        }
    }

    pub(super) fn send_conversation(&mut self) {
        let text = self.conversation_draft.trim().to_string();
        if text.is_empty() || self.conversation_inflight {
            return;
        }
        self.conversation_draft.clear();
        let _ =
            self.memory
                .record_event(&self.persona.id, EventKind::UserMessage, Some(text.clone()));
        self.remember_user_preferences(&text);
        self.conversation_history.push(ConversationTurn {
            user: true,
            text: text.clone(),
        });
        let context = self.memory.build_greeting_context(
            &self.persona.id,
            self.config.memory.recent_events,
            self.config.memory.fact_limit,
        );
        let history = self.conversation_history_text();
        let persona = self.persona.clone();
        let config = self.config.deepseek.clone();
        let now_text = greeting::local_now_text();
        let pet_name = self.pet.as_ref().map(|pet| pet.entry.display_name.clone());
        let pet_state = self
            .pet
            .as_ref()
            .map(|pet| pet.engine.current().name().to_string())
            .unwrap_or_else(|| PetState::Idle.name().to_string());
        let (sender, receiver) = mpsc::channel();
        self.conversation_rx = Some(receiver);
        self.conversation_inflight = true;
        let repaint_context = Arc::clone(&self.repaint_context);
        thread::spawn(move || {
            let result = DeepSeekClient::new(config)
                .and_then(|client| {
                    client.generate_reply(
                        &persona,
                        &context,
                        &history,
                        &text,
                        &now_text,
                        pet_name.as_deref(),
                        &pet_state,
                    )
                })
                .map_err(|error| format!("{error:#}"));
            let _ = sender.send(result);
            if let Some(ctx) = repaint_context
                .lock()
                .ok()
                .and_then(|context| context.clone())
            {
                ctx.request_repaint();
            }
        });
    }

    pub(super) fn poll_conversation(&mut self) {
        let received = self
            .conversation_rx
            .as_ref()
            .map(|receiver| receiver.try_recv());
        match received {
            Some(Ok(Ok(reply))) => {
                self.conversation_rx = None;
                self.conversation_inflight = false;
                self.conversation_history.push(ConversationTurn {
                    user: false,
                    text: reply.clone(),
                });
                let _ = self.memory.record_event(
                    &self.persona.id,
                    EventKind::PetReaction,
                    Some(reply.clone()),
                );
                self.show_bubble(reply);
            }
            Some(Ok(Err(error))) => {
                self.conversation_rx = None;
                self.conversation_inflight = false;
                self.status = error;
                let fallback = greeting::fallback_greeting(&self.persona);
                self.conversation_history.push(ConversationTurn {
                    user: false,
                    text: fallback.clone(),
                });
                let _ = self.memory.record_event(
                    &self.persona.id,
                    EventKind::PetReaction,
                    Some(fallback.clone()),
                );
                self.show_bubble(fallback);
            }
            Some(Err(TryRecvError::Empty)) | None => {}
            Some(Err(TryRecvError::Disconnected)) => {
                self.conversation_rx = None;
                self.conversation_inflight = false;
            }
        }
    }

    pub(super) fn show_conversation_viewport(
        &mut self,
        ctx: &egui::Context,
        frame: &eframe::Frame,
    ) {
        let conversation_id = egui::ViewportId::from_hash_of("petsona-conversation");
        let visible = self.conversation_open || self.conversation_closing_at.is_some();
        if !visible {
            self.conversation_shown_at = None;
            self.conversation_closing_at = None;
            self.conversation_cursor = None;
            if self.conversation_window_created {
                ctx.send_viewport_cmd_to(conversation_id, egui::ViewportCommand::Visible(false));
                self.conversation_window_created = false;
            }
            return;
        }
        let Some(window) = frame.winit_window() else {
            return;
        };
        let Ok(position) = window.outer_position() else {
            return;
        };
        let scale = window.scale_factor().max(0.1) as f32;
        let parent = egui::pos2(position.x as f32 / scale, position.y as f32 / scale);
        let pet_window = self.pet_window_size();
        // The overlay starts at the pet's top edge and includes the pet plus
        // the composer below it. Keeping both in one transparent viewport lets
        // the composer travel to the pet shadow without moving/resizing an OS
        // window every animation frame.
        let overlay_size = conversation_overlay_size(pet_window);
        let conversation_position = egui::pos2(
            parent.x + pet_window.x * 0.5 - overlay_size.x * 0.5,
            parent.y,
        );
        // First appearance: create the window hidden and let the shell disable
        // the system's show/hide transition before it is ever visible, so the
        // only motion is our own entry animation.
        if !self.conversation_window_warmed {
            let builder = egui::ViewportBuilder::default()
                .with_title(CONVERSATION_TITLE)
                .with_inner_size([overlay_size.x, overlay_size.y])
                .with_position([conversation_position.x, conversation_position.y])
                .with_transparent(true)
                .with_decorations(false)
                .with_always_on_top()
                .with_taskbar(false)
                .with_resizable(false)
                .with_active(true)
                .with_visible(false);
            ctx.show_viewport_immediate(conversation_id, builder, |_ui, _class| {});
            self.conversation_window_warmed = true;
            let _ = self
                .platform
                .prepare_activatable_popup_window(CONVERSATION_TITLE);
            return;
        }
        // Re-assert this every visible frame: winit can re-apply the decorated
        // style when it patches a viewport (for example while toggling mouse
        // passthrough or resizing after a pet-scale change).
        let _ = self
            .platform
            .prepare_activatable_popup_window(CONVERSATION_TITLE);

        let progress = if self.conversation_open {
            ease_out(entry_progress(
                self.conversation_shown_at,
                CONVERSATION_ENTRY,
            ))
        } else {
            smoothstep(entry_progress(
                self.conversation_closing_at,
                CONVERSATION_EXIT,
            ))
        };
        if !self.conversation_open && progress >= 1.0 {
            ctx.send_viewport_cmd_to(conversation_id, egui::ViewportCommand::Visible(false));
            self.conversation_window_created = false;
            self.conversation_closing_at = None;
            self.conversation_shown_at = None;
            self.conversation_cursor = None;
            return;
        }

        let pill_width = CONVERSATION_PILL_WIDTH.min(overlay_size.x - 24.0);
        let pill_rect = egui::Rect::from_center_size(
            egui::pos2(
                overlay_size.x * 0.5,
                pet_window.y + CONVERSATION_GAP + CONVERSATION_PILL_HEIGHT * 0.5,
            ),
            egui::vec2(pill_width, CONVERSATION_PILL_HEIGHT),
        );
        // The pet shadow is the origin/destination of the composer animation.
        let shadow_center = egui::pos2(
            overlay_size.x * 0.5,
            pet_window.y - CONVERSATION_SHADOW_Y_OFFSET,
        );
        let visual_scale = if self.conversation_open {
            lerp(CONVERSATION_COLLAPSED_SCALE, 1.0, progress)
        } else {
            lerp(1.0, CONVERSATION_COLLAPSED_SCALE, progress)
        };
        let opacity = if self.conversation_open {
            progress
        } else {
            1.0 - progress
        };
        let animated_center = if self.conversation_open {
            shadow_center.lerp(pill_rect.center(), progress)
        } else {
            pill_rect.center().lerp(shadow_center, progress)
        };
        let transform = egui::emath::TSTransform::new(
            animated_center.to_vec2() - pill_rect.center().to_vec2() * visual_scale,
            visual_scale,
        );
        let interactive = self.conversation_open && progress >= 1.0;
        let global_pill_rect = pill_rect.translate(conversation_position.to_vec2());
        let pointer_in_pill = self.pointer.position.is_some_and(|(x, y)| {
            global_pill_rect.contains(egui::pos2(x as f32 / scale, y as f32 / scale))
        });
        let builder = egui::ViewportBuilder::default()
            .with_title(CONVERSATION_TITLE)
            .with_inner_size([overlay_size.x, overlay_size.y])
            .with_position([conversation_position.x, conversation_position.y])
            .with_transparent(true)
            .with_decorations(false)
            .with_always_on_top()
            .with_taskbar(false)
            .with_resizable(false)
            .with_active(true)
            // Outside the resting pill, the transparent overlay must not
            // intercept clicks meant for the pet. Windows wakes on mouse move,
            // so this switches back to interactive before a click lands.
            .with_mouse_passthrough(!(interactive && pointer_in_pill))
            .with_visible(true);
        let mut send = false;
        let mut close = false;
        ctx.show_viewport_immediate(conversation_id, builder, |ui, _class| {
            // Capture these before TextEdit runs: it may consume Enter/Escape
            // while the input has focus.
            let enter_pressed =
                ui.input(|input| input.key_pressed(egui::Key::Enter) && !input.modifiers.shift);
            let escape_pressed =
                self.conversation_open && ui.input(|input| input.key_pressed(egui::Key::Escape));
            // The shadow fades in as the pill collapses and fades out as the
            // pill grows, giving the transition a clear point of origin.
            let shadow_alpha = if self.conversation_open {
                (1.0 - progress) * 0.55
            } else {
                progress * 0.55
            };
            if shadow_alpha > 0.001 {
                ui.painter().add(egui::Shape::ellipse_filled(
                    shadow_center,
                    egui::vec2(pill_width * 0.16, 4.0),
                    egui::Color32::from_rgba_unmultiplied(0, 0, 0, (shadow_alpha * 255.0) as u8),
                ));
            }

            ui.scope_builder(egui::UiBuilder::new().max_rect(pill_rect), |ui| {
                ui.set_opacity(opacity);
                ui.with_visual_transform(transform, |ui| {
                    if !self.conversation_open {
                        ui.disable();
                    }
                    egui::Frame::NONE
                        .fill(egui::Color32::from_rgb(28, 28, 32))
                        .stroke(egui::Stroke::new(
                            1.0,
                            egui::Color32::from_rgba_unmultiplied(255, 255, 255, 20),
                        ))
                        .corner_radius(18)
                        .inner_margin(egui::Margin::symmetric(8, 8))
                        .show(ui, |ui| {
                            ui.set_min_size(egui::vec2(pill_width - 16.0, 40.0));
                            ui.spacing_mut().item_spacing.x = 6.0;
                            ui.horizontal_centered(|ui| {
                                let input_width = (ui.available_width() - 42.0).max(80.0);
                                ui.allocate_ui_with_layout(
                                    egui::vec2(input_width, 36.0),
                                    egui::Layout::left_to_right(egui::Align::Center),
                                    |ui| {
                                        ui.set_min_height(36.0);
                                        let output = egui::TextEdit::singleline(
                                            &mut self.conversation_draft,
                                        )
                                        .id_salt("conversation-input")
                                        .desired_width(ui.available_width())
                                        .frame(egui::Frame::NONE)
                                        .margin(egui::Margin::symmetric(4, 8))
                                        .text_color(egui::Color32::from_rgb(245, 245, 247))
                                        .hint_text(
                                            egui::RichText::new("输入消息…")
                                                .color(egui::Color32::from_rgb(150, 150, 160)),
                                        )
                                        .show(ui);
                                        if let Some(cursor_range) = output.cursor_range {
                                            let cursor =
                                                output.galley.pos_from_cursor(cursor_range.primary);
                                            self.conversation_cursor = Some(
                                                conversation_position
                                                    + output.galley_pos.to_vec2()
                                                    + cursor.min.to_vec2(),
                                            );
                                        } else {
                                            self.conversation_cursor = Some(
                                                conversation_position
                                                    + output
                                                        .response
                                                        .response
                                                        .rect
                                                        .center()
                                                        .to_vec2(),
                                            );
                                        }

                                        if interactive && self.conversation_input_focus_pending {
                                            output.response.response.request_focus();
                                            self.conversation_input_focus_pending = false;
                                        }
                                        if interactive
                                            && output.response.response.has_focus()
                                            && enter_pressed
                                        {
                                            send = true;
                                        }
                                    },
                                );

                                let can_send = interactive
                                    && !self.conversation_inflight
                                    && !self.conversation_draft.trim().is_empty();
                                let arrow = egui::Button::new(
                                    egui::RichText::new("↑").size(20.0).color(if can_send {
                                        egui::Color32::WHITE
                                    } else {
                                        egui::Color32::from_rgba_unmultiplied(255, 255, 255, 90)
                                    }),
                                )
                                .fill(if can_send {
                                    egui::Color32::from_rgb(62, 142, 255)
                                } else {
                                    egui::Color32::from_rgba_unmultiplied(255, 255, 255, 16)
                                })
                                .stroke(egui::Stroke::NONE)
                                .corner_radius(18)
                                .min_size(egui::vec2(36.0, 36.0));
                                if ui.add_enabled(can_send, arrow).clicked() {
                                    send = true;
                                }
                            });
                        });
                });
            });
            if escape_pressed {
                close = true;
            }
        });
        self.conversation_window_created = true;
        if self.conversation_focus_pending {
            ctx.send_viewport_cmd_to(conversation_id, egui::ViewportCommand::Focus);
            self.conversation_focus_pending = false;
        }
        if send {
            self.send_conversation();
        }
        if close {
            self.close_conversation();
        }
    }
}

pub(super) fn conversation_overlay_size(pet_window: egui::Vec2) -> egui::Vec2 {
    egui::vec2(
        (CONVERSATION_PILL_WIDTH + CONVERSATION_OVERLAY_PADDING * 2.0)
            .max(pet_window.x + CONVERSATION_OVERLAY_PADDING * 2.0),
        pet_window.y + CONVERSATION_GAP + CONVERSATION_PILL_HEIGHT + CONVERSATION_OVERLAY_PADDING,
    )
}
