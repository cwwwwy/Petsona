using System.IO;
using System.Text.Json;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Media;
using System.Runtime.InteropServices.WindowsRuntime;
using Petsona.Core;
using Petsona.Core.Interop;
using Windows.Storage.Pickers;

namespace Petsona.Views;

/// <summary>
/// Settings surface (settings-consolidation REQ-S04–S07): six card-based pages
/// (pets / appearance / persona / memory / connection+greeting / system) that
/// apply every change immediately — there is no save button. Destructive actions
/// go through <see cref="ConfirmAsync"/>. The card look is hand-written; the
/// project intentionally avoids third-party UI packages.
/// </summary>
public sealed partial class SettingsWindow : Window
{
    private static readonly double[] ScalePresets = [0.5, 0.75, 1.0, 1.25, 1.5, 1.75, 2.0];
    private static readonly string[] VerbosityPresets = ["short", "normal", "detailed"];
    private static readonly JsonSerializerOptions JsonOptions = new() { PropertyNameCaseInsensitive = true };

    private const string RepositoryUrl = "https://github.com/cwwwwy/Petsona";
    private readonly EngineClient _engine;
    private readonly DispatcherQueueTimer _refreshTimer;
    private readonly DispatcherQueueTimer _applyTimer;
    private Action? _pendingApply;
    private readonly Dictionary<string, StackPanel> _pages;
    private readonly Dictionary<string, ImageSource?> _thumbnailCache = new();
    private string _lastPetsJson = string.Empty;
    private string _lastCodexJson = string.Empty;
    private string _lastPersonasJson = string.Empty;
    private string _lastMemoryJson = string.Empty;
    private string _lastConflictJson = string.Empty;
    private string _selectedPersonaId = string.Empty;
    private bool _formsLoaded;
    private bool _suppressEvents;

    public SettingsWindow(EngineClient engine)
    {
        _engine = engine;
        InitializeComponent();
        AppWindow.Resize(new Windows.Graphics.SizeInt32(1000, 720));
        _pages = new Dictionary<string, StackPanel>
        {
            ["pets"] = PagePets,
            ["appearance"] = PageAppearance,
            ["deepseek"] = PageDeepSeek,
            ["persona"] = PagePersona,
            ["memory"] = PageMemory,
            ["startup"] = PageStartup,
        };

        ScaleCombo.ItemsSource = ScalePresets.Select(value => $"{value:0.##}x").ToList();
        PersonaVerbosityCombo.ItemsSource = VerbosityPresets.ToList();

        _refreshTimer = DispatcherQueue.CreateTimer();
        _refreshTimer.Interval = TimeSpan.FromMilliseconds(500);
        _refreshTimer.IsRepeating = true;
        _refreshTimer.Tick += (_, _) => Refresh();
        _refreshTimer.Start();
        Closed += (_, _) => _refreshTimer.Stop();

        // Instant apply (REQ-S05): text boxes are debounced so a burst of
        // keystrokes turns into one command, everything else applies at once.
        _applyTimer = DispatcherQueue.CreateTimer();
        _applyTimer.Interval = TimeSpan.FromMilliseconds(450);
        _applyTimer.IsRepeating = false;
        _applyTimer.Tick += (_, _) =>
        {
            _applyTimer.Stop();
            var apply = _pendingApply;
            _pendingApply = null;
            apply?.Invoke();
        };

        DeepSeekModel.TextChanged += (_, _) => ScheduleApply(ApplyDeepSeekConfig);
        DeepSeekBaseUrl.TextChanged += (_, _) => ScheduleApply(ApplyDeepSeekConfig);
        DeepSeekApiKeyEnv.TextChanged += (_, _) => ScheduleApply(ApplyDeepSeekConfig);
        DeepSeekTimeout.ValueChanged += (_, _) => ScheduleApply(ApplyDeepSeekConfig);
        DeepSeekMaxTokens.ValueChanged += (_, _) => ScheduleApply(ApplyDeepSeekConfig);
        DeepSeekTemperature.ValueChanged += (_, _) => ScheduleApply(ApplyDeepSeekConfig);
        DeepSeekThinkingDisabled.Toggled += (_, _) => ScheduleApply(ApplyDeepSeekConfig);

        PersonaNameBox.TextChanged += (_, _) => ScheduleApply(ApplyPersona);
        PersonaDescriptionBox.TextChanged += (_, _) => ScheduleApply(ApplyPersona);
        PersonaToneBox.TextChanged += (_, _) => ScheduleApply(ApplyPersona);
        PersonaVerbosityCombo.SelectionChanged += (_, _) => ScheduleApply(ApplyPersona);
        PersonaLanguageBox.TextChanged += (_, _) => ScheduleApply(ApplyPersona);
        PersonaEmojiToggle.Toggled += (_, _) => ScheduleApply(ApplyPersona);
        PersonaGreetingBox.TextChanged += (_, _) => ScheduleApply(ApplyPersona);
        PersonaSystemPromptBox.TextChanged += (_, _) => ScheduleApply(ApplyPersona);

        MemoryEnabledToggle.Toggled += (_, _) => ScheduleApply(ApplyMemoryConfig);
        MemoryRecentEventsBox.ValueChanged += (_, _) => ScheduleApply(ApplyMemoryConfig);
        MemoryFactLimitBox.ValueChanged += (_, _) => ScheduleApply(ApplyMemoryConfig);

        GreetingEnabledToggle.Toggled += (_, _) => ScheduleApply(ApplyGreetingConfig);
        GreetingIdleMinutesBox.ValueChanged += (_, _) => ScheduleApply(ApplyGreetingConfig);
        GreetingCooldownBox.ValueChanged += (_, _) => ScheduleApply(ApplyGreetingConfig);
        GreetingMaxCharsBox.ValueChanged += (_, _) => ScheduleApply(ApplyGreetingConfig);

        AboutVersionText.Text = $"版本 {typeof(SettingsWindow).Assembly.GetName().Version?.ToString(3) ?? "0.1.0"} · 原生前端（WinUI 3 + Win32）";

        _suppressEvents = true;
        AutostartToggle.IsOn = SystemServices.IsAutostartEnabled();
        _suppressEvents = false;

        Refresh();
    }

    /// <summary>Give the settings surface keyboard focus once it is foreground.</summary>
    public void FocusRoot()
    {
        RootPanel.Focus(FocusState.Programmatic);
    }

    private void OnNavSelectionChanged(NavigationView sender, NavigationViewSelectionChangedEventArgs args)
    {
        if (args.SelectedItem is not NavigationViewItem item || item.Tag is not string tag)
        {
            return;
        }

        foreach (var (key, page) in _pages)
        {
            page.Visibility = key == tag ? Visibility.Visible : Visibility.Collapsed;
        }
    }

    // ---------------------------------------------------------------- refresh

    private void Refresh()
    {
        var petsJson = _engine.Text(PetsonaTextField.Pets);
        if (petsJson != _lastPetsJson)
        {
            _lastPetsJson = petsJson;
            _ = ReloadPetsAsync(petsJson);
        }

        var codexJson = _engine.Text(PetsonaTextField.CodexPets);
        if (codexJson != _lastCodexJson)
        {
            _lastCodexJson = codexJson;
            _ = ReloadCodexAsync(codexJson);
        }

        var personasJson = _engine.Text(PetsonaTextField.Personas);
        if (personasJson != _lastPersonasJson)
        {
            _lastPersonasJson = personasJson;
            var personas = Parse<List<PersonaEntry>>(personasJson) ?? [];
            if (_selectedPersonaId.Length == 0 || personas.All(persona => persona.Id != _selectedPersonaId))
            {
                _selectedPersonaId = _engine.Text(PetsonaTextField.PersonaId);
            }

            _suppressEvents = true;
            PersonaCombo.ItemsSource = personas;
            PersonaCombo.SelectedItem = personas.FirstOrDefault(persona => persona.Id == _selectedPersonaId);
            _suppressEvents = false;
        }

        var memoryJson = _engine.Text(PetsonaTextField.Memory);
        if (memoryJson != _lastMemoryJson)
        {
            _lastMemoryJson = memoryJson;
            FactsList.ItemsSource = ParseMemoryFacts(memoryJson);
            if (!_formsLoaded)
            {
                _formsLoaded = true;
                LoadScaleAndFlags();
                LoadDeepSeekForm();
                LoadMemoryForm(memoryJson);
                LoadPersonaForm();
            }
        }

        var conflict = _engine.Text(PetsonaTextField.ImportConflict);
        if (conflict != _lastConflictJson)
        {
            _lastConflictJson = conflict;
            if (string.IsNullOrEmpty(conflict))
            {
                ConflictPanel.Visibility = Visibility.Collapsed;
            }
            else
            {
                var entry = Parse<ConflictEntry>(conflict);
                ConflictText.Text = entry is null
                    ? $"本地已有同 ID 宠物，是否覆盖？{conflict}"
                    : $"本地已有同 ID 宠物「{entry.Name}」，是否覆盖导入？";
                ConflictPanel.Visibility = Visibility.Visible;
            }
        }

        var status = _engine.Text(PetsonaTextField.Status);
        StatusText.Text = string.IsNullOrEmpty(status)
            ? "选择要使用的宠物，或导入新的宠物包。"
            : status;
    }

    private void LoadScaleAndFlags()
    {
        var snapshot = _engine.Snapshot();
        _suppressEvents = true;
        var index = Array.FindIndex(ScalePresets, value => Math.Abs(value - snapshot.Scale) < 0.01);
        ScaleCombo.SelectedIndex = index < 0 ? Array.IndexOf(ScalePresets, 1.0) : index;
        ClickThroughToggle.IsOn = snapshot.ClickThrough != 0;
        _suppressEvents = false;
    }

    private void LoadDeepSeekForm()
    {
        using var document = ParseDocument(_engine.Text(PetsonaTextField.DeepSeekConfig));
        if (document is null)
        {
            return;
        }

        var root = document.RootElement;
        _suppressEvents = true;
        DeepSeekBaseUrl.Text = GetString(root, "baseUrl");
        DeepSeekModel.Text = GetString(root, "model");
        DeepSeekApiKeyEnv.Text = GetString(root, "apiKeyEnv");
        DeepSeekTimeout.Value = GetNumber(root, "timeoutSeconds", 30);
        DeepSeekMaxTokens.Value = GetNumber(root, "maxTokens", 256);
        DeepSeekTemperature.Value = GetNumber(root, "temperature", 0.7);
        DeepSeekThinkingDisabled.IsOn = GetBool(root, "thinkingDisabled", false);
        _suppressEvents = false;
    }

    private void LoadMemoryForm(string memoryJson)
    {
        using var document = ParseDocument(memoryJson);
        if (document is null)
        {
            return;
        }

        if (!TryGetObject(document.RootElement, "config", out var config))
        {
            return;
        }

        _suppressEvents = true;
        MemoryEnabledToggle.IsOn = GetBool(config, "enabled", true);
        MemoryRecentEventsBox.Value = GetNumber(config, "recentEvents", 10);
        MemoryFactLimitBox.Value = GetNumber(config, "factLimit", 50);

        if (TryGetObject(document.RootElement, "greeting", out var greeting))
        {
            GreetingEnabledToggle.IsOn = GetBool(greeting, "enabled", true);
            GreetingIdleMinutesBox.Value = GetNumber(greeting, "idleMinutes", 30);
            GreetingCooldownBox.Value = GetNumber(greeting, "cooldownMinutes", 120);
            GreetingMaxCharsBox.Value = GetNumber(greeting, "maxChars", 40);
        }

        _suppressEvents = false;
    }

    private void LoadPersonaForm()
    {
        using var document = ParseDocument(_engine.Text(PetsonaTextField.Persona));
        if (document is null)
        {
            return;
        }

        var root = document.RootElement;
        _suppressEvents = true;
        PersonaNameBox.Text = GetString(root, "name");
        PersonaDescriptionBox.Text = GetString(root, "description");
        PersonaGreetingBox.Text = GetString(root, "greeting");
        PersonaSystemPromptBox.Text = GetString(root, "systemPrompt");

        if (TryGetObject(root, "traits", out var traits))
        {
            PersonaToneBox.Text = GetString(traits, "tone");
            var verbosity = GetString(traits, "verbosity");
            PersonaVerbosityCombo.SelectedItem = VerbosityPresets.Contains(verbosity) ? verbosity : "normal";
            PersonaLanguageBox.Text = GetString(traits, "language");
            PersonaEmojiToggle.IsOn = GetBool(traits, "emoji", false);
        }

        PersonaIdBox.Text = $"人格 ID：{GetString(root, "id")}（导入 / 导出时使用）";

        KeyStatusText.Text = string.Empty;
        _suppressEvents = false;
    }

    // ------------------------------------------------------------- pets

    private void OnPetSelectionChanged(object sender, SelectionChangedEventArgs e)
    {
        if (!_suppressEvents && PetList.SelectedItem is PetListItem item)
        {
            _engine.Send(PetsonaCommandKind.SelectPet, text: item.Id);
        }
    }

    private async Task ReloadPetsAsync(string json)
    {
        var entries = Parse<List<PetEntry>>(json) ?? [];
        var items = new List<PetListItem>();
        foreach (var entry in entries)
        {
            items.Add(new PetListItem
            {
                Id = entry.Id,
                Label = entry.ToString(),
                Thumbnail = await LoadThumbnailAsync(entry.Spritesheet, entry.CellWidth, entry.CellHeight),
            });
        }

        PetList.ItemsSource = items;
    }

    private async Task ReloadCodexAsync(string json)
    {
        var entries = Parse<List<CodexPetEntry>>(json) ?? [];
        var items = new List<PetListItem>();
        foreach (var entry in entries)
        {
            items.Add(new PetListItem
            {
                Id = entry.Id,
                Label = entry.ToString(),
                Path = entry.Path,
                Thumbnail = await LoadThumbnailAsync(entry.Spritesheet, entry.CellWidth, entry.CellHeight),
            });
        }

        CodexList.ItemsSource = items;
    }

    private async Task<ImageSource?> LoadThumbnailAsync(string? path, uint cellWidth, uint cellHeight)
    {
        if (string.IsNullOrEmpty(path) || cellWidth == 0 || cellHeight == 0)
        {
            return null;
        }

        if (_thumbnailCache.TryGetValue(path, out var cached))
        {
            return cached;
        }

        try
        {
            var file = await Windows.Storage.StorageFile.GetFileFromPathAsync(path);
            using var stream = await file.OpenReadAsync();
            var decoder = await Windows.Graphics.Imaging.BitmapDecoder.CreateAsync(stream);
            // BitmapTransform produced corrupted pixels for webp sheets, so
            // decode the full image once and crop the first cell by hand.
            var pixelData = await decoder.GetPixelDataAsync(
                Windows.Graphics.Imaging.BitmapPixelFormat.Bgra8,
                Windows.Graphics.Imaging.BitmapAlphaMode.Premultiplied,
                new Windows.Graphics.Imaging.BitmapTransform(),
                Windows.Graphics.Imaging.ExifOrientationMode.IgnoreExifOrientation,
                Windows.Graphics.Imaging.ColorManagementMode.DoNotColorManage);
            var bytes = pixelData.DetachPixelData();
            var sheetWidth = (int)decoder.PixelWidth;
            var sheetHeight = (int)decoder.PixelHeight;
            var cropWidth = Math.Min((int)cellWidth, sheetWidth);
            var cropHeight = Math.Min((int)cellHeight, sheetHeight);
            var sourceStride = sheetWidth * 4;
            var cropStride = cropWidth * 4;
            var crop = new byte[cropWidth * cropHeight * 4];
            for (var row = 0; row < cropHeight; row++)
            {
                Array.Copy(bytes, row * sourceStride, crop, row * cropStride, cropStride);
            }

            var writeable = new Microsoft.UI.Xaml.Media.Imaging.WriteableBitmap(cropWidth, cropHeight);
            using (var pixelStream = writeable.PixelBuffer.AsStream())
            {
                pixelStream.Write(crop, 0, crop.Length);
            }

            writeable.Invalidate();
            _thumbnailCache[path] = writeable;
            return writeable;
        }
        catch (Exception error)
        {
            AppController.TryWriteErrorLog("thumbnail", error);
            _thumbnailCache[path] = null;
            return null;
        }
    }

    private async void OnImportFolderClick(object sender, RoutedEventArgs e)
    {
        var picker = new FolderPicker();
        picker.FileTypeFilter.Add("*");
        Initialize(picker);
        var folder = await picker.PickSingleFolderAsync();
        if (folder is not null)
        {
            _engine.Send(PetsonaCommandKind.ImportPet, text: folder.Path);
        }
    }

    private async void OnImportArchiveClick(object sender, RoutedEventArgs e)
    {
        var picker = new FileOpenPicker();
        picker.FileTypeFilter.Add(".zip");
        Initialize(picker);
        var file = await picker.PickSingleFileAsync();
        if (file is not null)
        {
            _engine.Send(PetsonaCommandKind.ImportPet, text: file.Path);
        }
    }

    private void OnScanCodexClick(object sender, RoutedEventArgs e)
    {
        _engine.Send(PetsonaCommandKind.ScanCodexPets);
    }

    private void OnCodexDoubleTapped(object sender, Microsoft.UI.Xaml.Input.DoubleTappedRoutedEventArgs e)
    {
        if (CodexList.SelectedItem is PetListItem { Path: { Length: > 0 } path })
        {
            _engine.Send(PetsonaCommandKind.ImportPet, text: path);
        }
    }

    private void OnOverwriteImportClick(object sender, RoutedEventArgs e)
    {
        var entry = Parse<ConflictEntry>(_engine.Text(PetsonaTextField.ImportConflict));
        if (entry is not null)
        {
            _engine.Send(PetsonaCommandKind.ImportPet, value: 1, text: entry.Path);
        }
    }

    private void OnCancelConflictClick(object sender, RoutedEventArgs e)
    {
        _engine.Send(PetsonaCommandKind.ClearImportConflict);
    }

    private async void OnExportPetClick(object sender, RoutedEventArgs e)
    {
        if (PetList.SelectedItem is not PetListItem entry)
        {
            return;
        }

        var picker = new FileSavePicker();
        picker.SuggestedFileName = entry.Id;
        picker.FileTypeChoices.Add("Petsona 宠物包", [".zip"]);
        Initialize(picker);
        var file = await picker.PickSaveFileAsync();
        if (file is not null)
        {
            _engine.Send(PetsonaCommandKind.ExportPet, text: entry.Id + "\n" + file.Path);
        }
    }

    private async void OnDeletePetClick(object sender, RoutedEventArgs e)
    {
        if (PetList.SelectedItem is not PetListItem entry)
        {
            return;
        }

        if (await ConfirmAsync("删除宠物", $"确定从本地库删除「{entry.Label}」吗？"))
        {
            _engine.Send(PetsonaCommandKind.DeletePet, text: entry.Id);
        }
    }

    // ------------------------------------------------------------ appearance

    private void OnScaleSelectionChanged(object sender, SelectionChangedEventArgs e)
    {
        if (_suppressEvents || ScaleCombo.SelectedIndex < 0)
        {
            return;
        }

        _engine.Send(PetsonaCommandKind.SetScale, value: ScalePresets[ScaleCombo.SelectedIndex]);
    }

    private void OnClickThroughToggled(object sender, RoutedEventArgs e)
    {
        if (!_suppressEvents)
        {
            _engine.Send(PetsonaCommandKind.SetClickThrough, value: ClickThroughToggle.IsOn ? 1 : 0);
        }
    }

    // ------------------------------------------------------------ deepseek

    private void ApplyDeepSeekConfig()
    {
        SendJson(PetsonaCommandKind.UpdateDeepSeekConfig, new Dictionary<string, object?>
        {
            ["baseUrl"] = DeepSeekBaseUrl.Text.Trim(),
            ["model"] = DeepSeekModel.Text.Trim(),
            ["apiKeyEnv"] = DeepSeekApiKeyEnv.Text.Trim(),
            ["timeoutSeconds"] = Number(DeepSeekTimeout, 30),
            ["maxTokens"] = (uint)Number(DeepSeekMaxTokens, 256),
            ["temperature"] = Number(DeepSeekTemperature, 0.7),
            ["thinkingDisabled"] = DeepSeekThinkingDisabled.IsOn,
        });
    }

    private void ApplyGreetingConfig()
    {
        SendJson(PetsonaCommandKind.UpdateGreetingConfig, new Dictionary<string, object?>
        {
            ["enabled"] = GreetingEnabledToggle.IsOn,
            ["idleMinutes"] = (uint)Number(GreetingIdleMinutesBox, 30),
            ["cooldownMinutes"] = (uint)Number(GreetingCooldownBox, 120),
            ["maxChars"] = (uint)Number(GreetingMaxCharsBox, 40),
        });
        _lastMemoryJson = string.Empty;
    }

    private void OnSaveKeyClick(object sender, RoutedEventArgs e)
    {
        var key = DeepSeekKeyBox.Password;
        if (key.Length == 0)
        {
            KeyStatusText.Text = "请输入 API Key";
            return;
        }

        _engine.Send(PetsonaCommandKind.SaveDeepSeekKey, text: key);
        DeepSeekKeyBox.Password = string.Empty;
        KeyStatusText.Text = "已发送保存请求";
    }

    private void OnClearKeyClick(object sender, RoutedEventArgs e)
    {
        _engine.Send(PetsonaCommandKind.SaveDeepSeekKey, text: string.Empty);
        KeyStatusText.Text = "已发送清除请求";
    }

    // ------------------------------------------------------------ personas

    private void OnPersonaSelectionChanged(object sender, SelectionChangedEventArgs e)
    {
        if (_suppressEvents || PersonaCombo.SelectedItem is not PersonaEntry entry)
        {
            return;
        }

        _selectedPersonaId = entry.Id;
        _engine.Send(PetsonaCommandKind.SelectPersona, text: entry.Id);
        ReloadPersonaSoon();
    }

    private void ApplyPersona()
    {
        SendJson(PetsonaCommandKind.UpdatePersona, new Dictionary<string, object?>
        {
            ["description"] = PersonaDescriptionBox.Text,
            ["name"] = PersonaNameBox.Text.Trim(),
            ["tone"] = PersonaToneBox.Text,
            ["verbosity"] = PersonaVerbosityCombo.SelectedItem as string ?? "normal",
            ["language"] = PersonaLanguageBox.Text.Trim(),
            ["emoji"] = PersonaEmojiToggle.IsOn,
            ["greeting"] = PersonaGreetingBox.Text,
            ["system_prompt"] = PersonaSystemPromptBox.Text,
        });
        _engine.Send(PetsonaCommandKind.SavePersona);
        _lastPersonasJson = string.Empty;
    }

    private async void OnCreatePersonaClick(object sender, RoutedEventArgs e)
    {
        var (id, name) = await AskPersonaIdentityAsync("新建人格", string.Empty, "新人格");
        if (id.Length == 0 || name.Length == 0)
        {
            return;
        }

        SendJson(PetsonaCommandKind.CreatePersona, new Dictionary<string, object?>
        {
            ["id"] = id,
            ["name"] = name,
        });
        _selectedPersonaId = id;
        _lastPersonasJson = string.Empty;
    }

    private async void OnDuplicatePersonaClick(object sender, RoutedEventArgs e)
    {
        if (PersonaCombo.SelectedItem is not PersonaEntry source)
        {
            return;
        }

        var (id, name) = await AskPersonaIdentityAsync("复制人格", string.Empty, source.Name + " 副本");
        if (id.Length == 0 || name.Length == 0)
        {
            return;
        }

        SendJson(PetsonaCommandKind.DuplicatePersona, new Dictionary<string, object?>
        {
            ["source_id"] = source.Id,
            ["id"] = id,
            ["name"] = name,
        });
        _selectedPersonaId = id;
        _lastPersonasJson = string.Empty;
    }

    private async void OnDeletePersonaClick(object sender, RoutedEventArgs e)
    {
        if (PersonaCombo.SelectedItem is not PersonaEntry entry)
        {
            return;
        }

        if (await ConfirmAsync("删除人格", $"确定删除人格「{entry.Name}」吗？内建人格不能删除。"))
        {
            _engine.Send(PetsonaCommandKind.DeletePersona, text: entry.Id);
            _lastPersonasJson = string.Empty;
        }
    }

    private async void OnImportPersonaClick(object sender, RoutedEventArgs e)
    {
        var picker = new FileOpenPicker();
        picker.FileTypeFilter.Add(".json");
        Initialize(picker);
        var file = await picker.PickSingleFileAsync();
        if (file is not null)
        {
            _engine.Send(PetsonaCommandKind.ImportPersona, text: file.Path);
        }
    }

    private async void OnExportPersonaClick(object sender, RoutedEventArgs e)
    {
        if (PersonaCombo.SelectedItem is not PersonaEntry entry)
        {
            return;
        }

        var picker = new FileSavePicker();
        picker.SuggestedFileName = entry.Id;
        picker.FileTypeChoices.Add("人格 JSON", [".json"]);
        Initialize(picker);
        var file = await picker.PickSaveFileAsync();
        if (file is not null)
        {
            _engine.Send(PetsonaCommandKind.ExportPersona, text: entry.Id + "\n" + file.Path);
        }
    }

    private void ReloadPersonaSoon()
    {
        _lastPersonasJson = string.Empty;
        _ = DispatcherQueue.TryEnqueue(async () =>
        {
            await Task.Delay(200);
            LoadPersonaForm();
        });
    }

    // ------------------------------------------------------------- memory

    private void ApplyMemoryConfig()
    {
        SendJson(PetsonaCommandKind.UpdateMemoryConfig, new Dictionary<string, object?>
        {
            ["enabled"] = MemoryEnabledToggle.IsOn,
            ["recentEvents"] = (uint)Number(MemoryRecentEventsBox, 10),
            ["factLimit"] = (uint)Number(MemoryFactLimitBox, 50),
        });
        _lastMemoryJson = string.Empty;
    }

    private void OnRememberFactClick(object sender, RoutedEventArgs e)
    {
        var key = FactKeyBox.Text.Trim();
        var value = FactValueBox.Text.Trim();
        if (key.Length == 0 || value.Length == 0)
        {
            return;
        }

        SendJson(PetsonaCommandKind.RememberFact, new Dictionary<string, object?>
        {
            ["key"] = key,
            ["value"] = value,
        });
        FactKeyBox.Text = string.Empty;
        FactValueBox.Text = string.Empty;
        _lastMemoryJson = string.Empty;
    }

    private void OnForgetFactClick(object sender, RoutedEventArgs e)
    {
        if (FactsList.SelectedItem is FactEntry entry)
        {
            _engine.Send(PetsonaCommandKind.ForgetFact, text: entry.Id);
            _lastMemoryJson = string.Empty;
        }
    }

    private async void OnClearMemoryClick(object sender, RoutedEventArgs e)
    {
        if (await ConfirmAsync("清空记忆", "确定清空当前人格的事实与事件记忆吗？"))
        {
            _engine.Send(PetsonaCommandKind.ClearMemory);
            _lastMemoryJson = string.Empty;
        }
    }

    // ------------------------------------------------------------ autostart

    private void OnAutostartToggled(object sender, RoutedEventArgs e)
    {
        if (_suppressEvents)
        {
            return;
        }

        try
        {
            SystemServices.SetAutostart(AutostartToggle.IsOn);
            StatusText.Text = AutostartToggle.IsOn ? "已启用开机自启动。" : "已关闭开机自启动。";
        }
        catch (Exception ex) when (ex is UnauthorizedAccessException or InvalidOperationException)
        {
            _suppressEvents = true;
            AutostartToggle.IsOn = SystemServices.IsAutostartEnabled();
            _suppressEvents = false;
            StatusText.Text = $"开机自启动设置失败：{ex.Message}";
        }
    }

    // --------------------------------------------------------- about / data

    private void OnOpenDataFolderClick(object sender, RoutedEventArgs e)
    {
        OpenFolder(DataDirectory());
    }

    private void OnOpenLogFolderClick(object sender, RoutedEventArgs e)
    {
        OpenFolder(Path.Combine(DataDirectory(), "logs"));
    }

    private void OnOpenRepoClick(object sender, RoutedEventArgs e)
    {
        OpenFolder(RepositoryUrl);
    }

    private static string DataDirectory()
    {
        var configured = Environment.GetEnvironmentVariable("PETSONA_HOME");
        return string.IsNullOrWhiteSpace(configured)
            ? Path.Combine(Environment.GetFolderPath(Environment.SpecialFolder.ApplicationData), "Petsona")
            : configured;
    }

    private static void OpenFolder(string target)
    {
        try
        {
            System.Diagnostics.Process.Start(new System.Diagnostics.ProcessStartInfo(target)
            {
                UseShellExecute = true,
            });
        }
        catch (Exception)
        {
            // Opening the folder is a convenience; never break settings on failure.
        }
    }

    // ------------------------------------------------------------- helpers

    /// <summary>Debounce a change into one immediate command (REQ-S05).</summary>
    private void ScheduleApply(Action apply)
    {
        if (_suppressEvents)
        {
            return;
        }

        _pendingApply = apply;
        _applyTimer.Stop();
        _applyTimer.Start();
    }

    private void SendJson(PetsonaCommandKind kind, Dictionary<string, object?> payload)
    {
        _engine.Send(kind, text: JsonSerializer.Serialize(payload));
    }

    private static double Number(NumberBox box, double fallback)
    {
        return double.IsNaN(box.Value) ? fallback : box.Value;
    }

    private void Initialize(object picker)
    {
        var handle = WinRT.Interop.WindowNative.GetWindowHandle(this);
        WinRT.Interop.InitializeWithWindow.Initialize(picker, handle);
    }

    private async Task<bool> ConfirmAsync(string title, string message)
    {
        var dialog = new ContentDialog
        {
            Title = title,
            Content = message,
            PrimaryButtonText = "确认",
            CloseButtonText = "取消",
            DefaultButton = ContentDialogButton.Close,
            XamlRoot = RootPanel.XamlRoot,
        };
        return await dialog.ShowAsync() == ContentDialogResult.Primary;
    }

    private async Task<(string Id, string Name)> AskPersonaIdentityAsync(string title, string id, string name)
    {
        var idBox = new TextBox { Header = "人格 ID（英文、数字或短横线）", Text = id };
        var nameBox = new TextBox { Header = "名称", Text = name };
        var panel = new StackPanel { Spacing = 8 };
        panel.Children.Add(idBox);
        panel.Children.Add(nameBox);
        var dialog = new ContentDialog
        {
            Title = title,
            Content = panel,
            PrimaryButtonText = "创建",
            CloseButtonText = "取消",
            DefaultButton = ContentDialogButton.Primary,
            XamlRoot = RootPanel.XamlRoot,
        };
        if (await dialog.ShowAsync() != ContentDialogResult.Primary)
        {
            return (string.Empty, string.Empty);
        }

        return (idBox.Text.Trim(), nameBox.Text.Trim());
    }

    private static List<FactEntry> ParseMemoryFacts(string json)
    {
        using var document = ParseDocument(json);
        if (document is null || !document.RootElement.TryGetProperty("facts", out var facts) ||
            facts.ValueKind != JsonValueKind.Array)
        {
            return [];
        }

        var result = new List<FactEntry>();
        foreach (var fact in facts.EnumerateArray())
        {
            result.Add(new FactEntry
            {
                Id = GetString(fact, "id"),
                Key = GetString(fact, "key"),
                Value = GetString(fact, "value"),
            });
        }

        return result;
    }

    private static T? Parse<T>(string json)
    {
        if (string.IsNullOrWhiteSpace(json))
        {
            return default;
        }

        try
        {
            return JsonSerializer.Deserialize<T>(json, JsonOptions);
        }
        catch (JsonException)
        {
            return default;
        }
    }

    private static JsonDocument? ParseDocument(string json)
    {
        if (string.IsNullOrWhiteSpace(json))
        {
            return null;
        }

        try
        {
            return JsonDocument.Parse(json);
        }
        catch (JsonException)
        {
            return null;
        }
    }

    private static string GetString(JsonElement element, string name)
    {
        return element.TryGetProperty(name, out var value) && value.ValueKind == JsonValueKind.String
            ? value.GetString() ?? string.Empty
            : string.Empty;
    }

    private static double GetNumber(JsonElement element, string name, double fallback)
    {
        return element.TryGetProperty(name, out var value) && value.ValueKind == JsonValueKind.Number
            ? value.GetDouble()
            : fallback;
    }

    private static bool GetBool(JsonElement element, string name, bool fallback)
    {
        return element.TryGetProperty(name, out var value) && value.ValueKind is JsonValueKind.True or JsonValueKind.False
            ? value.GetBoolean()
            : fallback;
    }

    private static bool TryGetObject(JsonElement element, string name, out JsonElement value)
    {
        if (element.TryGetProperty(name, out value) && value.ValueKind == JsonValueKind.Object)
        {
            return true;
        }

        value = default;
        return false;
    }

    private sealed record PetEntry(string Id, string Name, bool V2, string? Spritesheet, uint CellWidth, uint CellHeight)
    {
        public override string ToString() => V2 ? $"{Name}（{Id}，V2）" : $"{Name}（{Id}）";
    }

    private sealed record CodexPetEntry(string Id, string Name, string Path, string? Spritesheet, uint CellWidth, uint CellHeight)
    {
        public override string ToString() => $"{Name}（{Id}）";
    }

    private sealed record PersonaEntry(string Id, string Name, string? Description, bool Builtin)
    {
        public override string ToString() => Builtin ? $"{Name}（内建）" : Name;
    }

    private sealed record ConflictEntry(string Id, string Name, string Path)
    {
        public override string ToString() => $"{Name}（{Id}）";
    }

    private sealed class FactEntry
    {
        public required string Id { get; init; }

        public required string Key { get; init; }

        public required string Value { get; init; }

        public override string ToString() => $"{Key}：{Value}";
    }
}
