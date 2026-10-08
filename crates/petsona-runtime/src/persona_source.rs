use petsona_core::persona::{PersonaSource, PersonaStyleProfile};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonaDraft {
    pub id: String,
    pub name: String,
    pub style: PersonaStyleProfile,
    pub source: PersonaSource,
}

pub struct ActivePersonaGeneration {
    pub request_id: String,
    pub pet_id: String,
    pub persona_id: String,
    pub persona_json: String,
    pub kind: String,
    pub label: String,
    pub description: String,
    pub target_speaker_label: String,
    pub sample_count: usize,
}

pub struct ActivePersonaPreview {
    pub request_id: String,
    pub pet_id: String,
    pub draft_id: String,
}
