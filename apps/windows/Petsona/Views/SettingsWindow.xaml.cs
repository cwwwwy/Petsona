using System.IO;
using System.Reflection;
using System.Text.Json;
using Microsoft.UI.Dispatching;
using Microsoft.UI.Xaml;
using Microsoft.UI.Xaml.Controls;
using Microsoft.UI.Xaml.Controls.Primitives;
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
    /// The slider snaps to these steps and the tray / status menu uses the same
    /// set, so both entry points stay consistent (REQ-S12).
    private const double ScaleMin = 0.5;
    private const double ScaleMax = 2.0;
    private const double ScaleStep = 0.25;

    /// Tone presets carry both the style text and the reply length, so the
    /// persona page needs no separate verbosity control (REQ-S13/S14).
    private static readonly (string Label, string Tone, string Verbosity)[] TonePresets =
    [
        ("温和友好", "温和、友好、乐于帮忙", "normal"),
        ("简洁干练", "简洁、直接、不说废话", "short"),
        ("活泼元气", "活泼、元气满满、鼓励式回应", "normal"),
        ("沉稳顾问", "沉稳、克制、结构化", "detailed"),
        ("毒舌但温柔", "毒舌但温柔，吐槽背后是真的关心", "short"),
    ];
    private const string CustomToneLabel = "自定义…";

    /// Provider presets: the built-in DeepSeek endpoint or any OpenAI-compatible
    /// one. Only DeepSeek accepts the `thinking` field (REQ-S16).
    private static readonly (string Label, string Value)[] ProviderPresets =
    [
        ("DeepSeek", "deepseek"),
        ("自定义", "custom"),
    ];
    private const string DeepSeekBaseUrlPreset = "https://api.deepseek.com/v1";
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
    private string _lastModelsJson = string.Empty;
    private string _lastDeepSeekJson = string.Empty;
    private string _lastConflictJson = string.Empty;
    private bool _formsLoaded;
    private bool _suppressEvents;
    private string _personaVerbosity = "normal";
    private string _editingFactId = string.Empty;

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

        PersonaTonePresetCombo.ItemsSource = TonePresets
            .Select(preset => preset.Label)
            .Append(CustomToneLabel)
            .ToList();

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

        PersonaToneBox.TextChanged += (_, _) => ScheduleApply(ApplyPersona);
        PersonaEmojiToggle.Toggled += (_, _) => ScheduleApply(ApplyPersona);
        PersonaGreetingBox.TextChanged += (_, _) => ScheduleApply(ApplyPersona);
        PersonaSystemPromptBox.TextChanged += (_, _) => ScheduleApply(ApplyPersona);

        MemoryEnabledToggle.Toggled += (_, _) => ScheduleApply(ApplyMemoryConfig);
        MemoryRecentEventsBox.ValueChanged += (_, _) => ScheduleApply(ApplyMemoryConfig);
        MemoryFactLimitBox.ValueChanged += (_, _) => ScheduleApply(ApplyMemoryConfig);
        MemoryRetentionBox.ValueChanged += (_, _) => ScheduleApply(ApplyMemoryConfig);
        MemoryCompressToggle.Toggled += (_, _) => ScheduleApply(ApplyMemoryConfig);

        GreetingEnabledToggle.Toggled += (_, _) => ScheduleApply(ApplyGreetingConfig);
        GreetingIdleMinutesBox.ValueChanged += (_, _) => ScheduleApply(ApplyGreetingConfig);
        GreetingCooldownBox.ValueChanged += (_, _) => ScheduleApply(ApplyGreetingConfig);
        GreetingMaxCharsBox.ValueChanged += (_, _) => ScheduleApply(ApplyGreetingConfig);

        ProviderCombo.ItemsSource = ProviderPresets.Select(preset => preset.Label).ToList();

        AboutVersionText.Text = $"版本 {AppVersion()} · 原生前端（WinUI 3 + Win32）";

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

        var deepSeekJson = _engine.Text(PetsonaTextField.DeepSeekConfig);
        if (deepSeekJson != _lastDeepSeekJson)
        {
            _lastDeepSeekJson = deepSeekJson;
            ApplyKeyStatus(deepSeekJson);
        }

        var modelsJson = _engine.Text(PetsonaTextField.Models);
        if (modelsJson != _lastModelsJson)
        {
            _lastModelsJson = modelsJson;
            var models = Parse<List<string>>(modelsJson) ?? [];
            _suppressEvents = true;
            ModelCombo.ItemsSource = models;
            ModelCombo.Visibility = models.Count > 0 ? Visibility.Visible : Visibility.Collapsed;
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
        UpdateModelFetchStatus(status);
    }

    private void LoadScaleAndFlags()
    {
        var snapshot = _engine.Snapshot();
        _suppressEvents = true;
        var scale = Math.Clamp(snapshot.Scale, ScaleMin, ScaleMax);
        ScaleSlider.Value = Math.Round(scale / ScaleStep) * ScaleStep;
        ScaleValueText.Text = $"{ScaleSlider.Value:0.##}x";
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
        var provider = GetString(root, "provider") == "custom" ? "custom" : "deepseek";
        ProviderCombo.SelectedIndex = provider == "custom" ? 1 : 0;
        ApplyProviderUi(provider);
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
        MemoryRetentionBox.Value = GetNumber(config, "eventRetentionDays", 0);
        MemoryCompressToggle.IsOn = GetBool(config, "factCompress", true);

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
        PersonaGreetingBox.Text = GetString(root, "greeting");
        PersonaSystemPromptBox.Text = GetString(root, "systemPrompt");

        if (TryGetObject(root, "traits", out var traits))
        {
            var tone = GetString(traits, "tone");
            PersonaToneBox.Text = tone;
            _personaVerbosity = GetString(traits, "verbosity") is { Length: > 0 } verbosity
                ? verbosity
                : "normal";
            PersonaTonePresetCombo.SelectedIndex = FindTonePreset(tone);
            PersonaEmojiToggle.IsOn = GetBool(traits, "emoji", true);
        }

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

    /// Slider steps are discrete, so every change is a real user choice: the
    /// window resizes live and the runtime persists it (REQ-S12).
    private void OnScaleChanged(object sender, RangeBaseValueChangedEventArgs e)
    {
        // XAML applies Minimum/Maximum while the tree is still being built, which
        // can raise ValueChanged before the label exists (settings window would
        // fail to open). Guard both.
        if (_suppressEvents || ScaleValueText is null || ScaleSlider is null)
        {
            return;
        }

        ScaleValueText.Text = $"{ScaleSlider.Value:0.##}x";
        _engine.Send(PetsonaCommandKind.SetScale, value: ScaleSlider.Value);
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
            ["provider"] = CurrentProvider,
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

    /// The provider decides whether the base URL is a preset and whether the
    /// DeepSeek-only thinking switch is relevant (REQ-S16).
    private void ApplyProviderUi(string provider)
    {
        if (DeepSeekBaseUrl is null || DeepSeekBaseUrlHint is null || DeepSeekThinkingDisabled is null)
        {
            return;
        }

        var isDeepSeek = provider != "custom";
        DeepSeekBaseUrl.IsReadOnly = isDeepSeek;
        if (isDeepSeek)
        {
            DeepSeekBaseUrl.Text = DeepSeekBaseUrlPreset;
        }

        DeepSeekBaseUrlHint.Text = isDeepSeek
            ? "DeepSeek 的固定地址；切换到「自定义」后可自行填写。"
            : "自定义端点的地址，通常以 /v1 结尾。";
        DeepSeekThinkingDisabled.Visibility = isDeepSeek ? Visibility.Visible : Visibility.Collapsed;
    }

    private string CurrentProvider => ProviderCombo?.SelectedIndex == 1 ? "custom" : "deepseek";

    private void OnProviderSelectionChanged(object sender, SelectionChangedEventArgs e)
    {
        if (_suppressEvents || ProviderCombo is null)
        {
            return;
        }

        _suppressEvents = true;
        ApplyProviderUi(CurrentProvider);
        _suppressEvents = false;
        ScheduleApply(ApplyDeepSeekConfig);
    }

    /// Show whether a credential exists without ever displaying the secret
    /// (W19 feedback). The runtime recomputes the flag when the key or the
    /// provider changes, so this stays correct after 保存 / 清除.
    private void ApplyKeyStatus(string deepSeekJson)
    {
        if (KeyStatusText is null || ClearKeyButton is null || DeepSeekKeyBox is null)
        {
            return;
        }

        using var document = ParseDocument(deepSeekJson);
        var configured = document is not null &&
            GetBool(document.RootElement, "keyConfigured", false);
        KeyStatusText.Text = configured ? "已配置（密钥不会显示）" : "未配置";
        ClearKeyButton.IsEnabled = configured;
        DeepSeekKeyBox.PlaceholderText = configured
            ? "已配置：输入新密钥可覆盖"
            : "sk-...";
    }

    /// A failed model fetch must be visible where the user clicked, not only in
    /// the window's status bar (W19 feedback).
    private void UpdateModelFetchStatus(string status)
    {
        if (ModelFetchStatus is null)
        {
            return;
        }

        var failed = status.StartsWith("拉取模型列表失败", StringComparison.Ordinal) ||
                     status.StartsWith("服务商没有返回", StringComparison.Ordinal);
        if (failed)
        {
            ModelFetchStatus.Text = status;
            ModelFetchStatus.Foreground = new SolidColorBrush(Microsoft.UI.Colors.IndianRed);
            ModelFetchStatus.Visibility = Visibility.Visible;
        }
        else
        {
            ModelFetchStatus.Visibility = Visibility.Collapsed;
        }
    }

    private void OnListModelsClick(object sender, RoutedEventArgs e)
    {
        _engine.Send(PetsonaCommandKind.ListModels);
    }

    /// Picking a fetched model just fills the text field; the normal debounced
    /// apply persists it (REQ-S16b).
    private void OnModelComboChanged(object sender, SelectionChangedEventArgs e)
    {
        if (_suppressEvents || ModelCombo.SelectedItem is not string model || model.Length == 0)
        {
            return;
        }

        _suppressEvents = true;
        DeepSeekModel.Text = model;
        _suppressEvents = false;
        ScheduleApply(ApplyDeepSeekConfig);
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

    private void OnPersonaTonePresetChanged(object sender, SelectionChangedEventArgs e)
    {
        if (_suppressEvents || PersonaTonePresetCombo.SelectedIndex < 0)
        {
            return;
        }

        var index = PersonaTonePresetCombo.SelectedIndex;
        if (index >= TonePresets.Length)
        {
            return; // "自定义…" keeps whatever the user typed
        }

        _suppressEvents = true;
        PersonaToneBox.Text = TonePresets[index].Tone;
        _personaVerbosity = TonePresets[index].Verbosity;
        _suppressEvents = false;
        ScheduleApply(ApplyPersona);
    }

    /// Preset index, or the "custom" entry when the tone is not one of them.
    private static int FindTonePreset(string tone)
    {
        for (var index = 0; index < TonePresets.Length; index++)
        {
            if (string.Equals(TonePresets[index].Tone, tone.Trim(), StringComparison.Ordinal))
            {
                return index;
            }
        }

        return TonePresets.Length;
    }

    private void ApplyPersona()
    {
        SendJson(PetsonaCommandKind.UpdatePersona, new Dictionary<string, object?>
        {
            ["tone"] = PersonaToneBox.Text,
            ["verbosity"] = _personaVerbosity,
            ["emoji"] = PersonaEmojiToggle.IsOn,
            ["greeting"] = PersonaGreetingBox.Text,
            ["system_prompt"] = PersonaSystemPromptBox.Text,
        });
        _engine.Send(PetsonaCommandKind.SavePersona);
        _lastPersonasJson = string.Empty;
    }

    private async void OnResetPersonaClick(object sender, RoutedEventArgs e)
    {
        if (await ConfirmAsync("重置说话方式", "这会恢复内置的语气、emoji 与提示词，当前宠物的记忆不受影响。确定继续吗？"))
        {
            _engine.Send(PetsonaCommandKind.ResetPersona);
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
        var id = _engine.Text(PetsonaTextField.PersonaId);
        if (id.Length == 0)
        {
            return;
        }

        var picker = new FileSavePicker();
        picker.SuggestedFileName = id;
        picker.FileTypeChoices.Add("说话方式 JSON", [".json"]);
        Initialize(picker);
        var file = await picker.PickSaveFileAsync();
        if (file is not null)
        {
            _engine.Send(PetsonaCommandKind.ExportPersona, text: id + "\n" + file.Path);
        }
    }

    // ------------------------------------------------------------- memory

    private void ApplyMemoryConfig()
    {
        SendJson(PetsonaCommandKind.UpdateMemoryConfig, new Dictionary<string, object?>
        {
            ["enabled"] = MemoryEnabledToggle.IsOn,
            ["recentEvents"] = (uint)Number(MemoryRecentEventsBox, 10),
            ["factLimit"] = (uint)Number(MemoryFactLimitBox, 50),
            ["eventRetentionDays"] = (uint)Number(MemoryRetentionBox, 0),
            ["factCompress"] = MemoryCompressToggle.IsOn,
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

        if (_editingFactId.Length > 0)
        {
            SendJson(PetsonaCommandKind.UpdateMemoryFact, new Dictionary<string, object?>
            {
                ["id"] = _editingFactId,
                ["key"] = key,
                ["value"] = value,
            });
        }
        else
        {
            SendJson(PetsonaCommandKind.RememberFact, new Dictionary<string, object?>
            {
                ["key"] = key,
                ["value"] = value,
            });
        }

        ResetFactEditor();
        _lastMemoryJson = string.Empty;
    }

    /// Selecting a fact loads it into the editor; the same button then updates
    /// it in place instead of creating a duplicate (REQ-S15).
    private void OnFactSelectionChanged(object sender, SelectionChangedEventArgs e)
    {
        if (_suppressEvents || FactsList.SelectedItem is not FactEntry entry)
        {
            return;
        }

        _editingFactId = entry.Id;
        _suppressEvents = true;
        FactKeyBox.Text = entry.Key;
        FactValueBox.Text = entry.Value;
        _suppressEvents = false;
        FactSubmitButton.Content = "更新事实";
        FactCancelButton.Visibility = Visibility.Visible;
    }

    private void OnCancelFactEditClick(object sender, RoutedEventArgs e)
    {
        ResetFactEditor();
    }

    private void ResetFactEditor()
    {
        _editingFactId = string.Empty;
        _suppressEvents = true;
        FactKeyBox.Text = string.Empty;
        FactValueBox.Text = string.Empty;
        FactsList.SelectedItem = null;
        _suppressEvents = false;
        FactSubmitButton.Content = "添加事实";
        FactCancelButton.Visibility = Visibility.Collapsed;
    }

    private void OnForgetFactClick(object sender, RoutedEventArgs e)
    {
        if (FactsList.SelectedItem is FactEntry entry)
        {
            _engine.Send(PetsonaCommandKind.ForgetFact, text: entry.Id);
            ResetFactEditor();
            _lastMemoryJson = string.Empty;
        }
    }

    private void OnClearFactsClick(object sender, RoutedEventArgs e)
    {
        _engine.Send(PetsonaCommandKind.ClearMemoryScope, value: 1);
        ResetFactEditor();
        _lastMemoryJson = string.Empty;
    }

    private void OnClearEventsClick(object sender, RoutedEventArgs e)
    {
        _engine.Send(PetsonaCommandKind.ClearMemoryScope, value: 2);
        _lastMemoryJson = string.Empty;
    }

    private async void OnExportMemoryClick(object sender, RoutedEventArgs e)
    {
        var picker = new FileSavePicker();
        picker.FileTypeChoices.Add("记忆文件", new List<string> { ".json" });
        picker.SuggestedFileName = "petsona-memory";
        Initialize(picker);
        var file = await picker.PickSaveFileAsync();
        if (file is not null)
        {
            _engine.Send(PetsonaCommandKind.ExportMemory, text: file.Path);
        }
    }

    private async void OnImportMemoryClick(object sender, RoutedEventArgs e)
    {
        var picker = new FileOpenPicker();
        picker.FileTypeFilter.Add(".json");
        Initialize(picker);
        var file = await picker.PickSingleFileAsync();
        if (file is null)
        {
            return;
        }

        if (await ConfirmAsync("导入记忆", "导入会用文件内容覆盖当前人格的偏好与事件，确定继续吗？"))
        {
            _engine.Send(PetsonaCommandKind.ImportMemory, text: file.Path);
            ResetFactEditor();
            _lastMemoryJson = string.Empty;
        }
    }

    private async void OnClearMemoryClick(object sender, RoutedEventArgs e)
    {
        if (await ConfirmAsync("全部清空", "确定清空当前人格的偏好与事件记忆吗？"))
        {
            _engine.Send(PetsonaCommandKind.ClearMemory);
            ResetFactEditor();
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

    /// <summary>
    /// Informational version keeps the pre-release suffix (0.1.0-rc.1), which
    /// <see cref="System.Reflection.AssemblyName.Version"/> would drop — the
    /// settings page showed a stale number before (bug report 2026-09-22).
    /// </summary>
    private static string AppVersion()
    {
        var assembly = typeof(SettingsWindow).Assembly;
        var informational = assembly
            .GetCustomAttribute<System.Reflection.AssemblyInformationalVersionAttribute>()
            ?.InformationalVersion;
        if (!string.IsNullOrEmpty(informational))
        {
            // Strip the "+<commit>" build metadata when present.
            var plus = informational.IndexOf('+');
            return plus > 0 ? informational[..plus] : informational;
        }

        return assembly.GetName().Version?.ToString(3) ?? "0.0.0";
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
                Source = FactSourceLabel(GetString(fact, "source")),
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

    private static string FactSourceLabel(string source) => source switch
    {
        "conversation" => "对话",
        "import" => "导入",
        "compressed" => "压缩",
        _ => "手动",
    };

    private sealed class FactEntry
    {
        public required string Id { get; init; }

        public required string Key { get; init; }

        public required string Value { get; init; }

        public required string Source { get; init; }

        public string Label => $"{Key}：{Value}";

        public string Meta => $"来源：{Source}";

        public override string ToString() => Label;
    }
}
