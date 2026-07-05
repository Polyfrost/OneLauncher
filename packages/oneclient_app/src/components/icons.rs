use freya::prelude::*;
use oneclient_core::packages::ProviderId;

use crate::{AppAssets, theme::colors};

#[derive(PartialEq)]
pub struct Icon {
    pub icon: IconType,
    pub size_px: f32,
    pub color: Option<Color>,
}

impl Icon {
    pub fn new(icon: impl Into<IconType>) -> Self {
        let icon = icon.into();
        let color = match icon {
            IconType::Modrinth => Some(colors::MODRINTH_COLOR),
            IconType::Curseforge => Some(colors::CURSEFORGE_COLOR),
            _ => None,
        };

        Self {
            icon,
            size_px: 24.,
            color,
        }
    }

    pub fn size(mut self, size_px: f32) -> Self {
        self.size_px = size_px;
        self
    }

    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }
}

impl Component for Icon {
    fn render(&self) -> impl IntoElement {
        let path = self.icon.path();
        let bytes = use_memo(move || AppAssets::get_bytes(path).unwrap_or_default());

        svg(bytes.read().cloned())
            .width(Size::px(self.size_px))
            .height(Size::px(self.size_px))
            // .fill(color)
            .map(self.color, |svg, color| svg.color(color))
    }
}

impl From<ProviderId> for IconType {
    fn from(val: ProviderId) -> Self {
        match val {
            ProviderId::Modrinth => IconType::Modrinth,
            ProviderId::CurseForge => IconType::Curseforge,
            ProviderId::Local => IconType::Folder,
        }
    }
}

#[derive(oneclient_macro::IconNamed, Debug, PartialEq, Clone, Copy)]
#[allow(dead_code)]
pub enum IconType {
    AlertCircle,
    AlertTriangle,
    Announcement01,
    ArrowLeft,
    ArrowRight,
    Brush01,
    Bell01,
    Calendar,
    CheckCircle,
    Check,
    ChevronDown,
    ChevronRight,
    ChevronsLeft,
    ChevronsRight,
    ClipboardCheck,
    ClockRewind,
    CodeSnippet02,
    Colors,
    Copy01,
    Curseforge,
    Database01,
    DotsGrid,
    DotsVertical,
    Download01,
    DownloadCloud02,
    Eye,
    File02,
    FilePlus02,
    FileX02,
    Folder,
    FolderCheck,
    FolderDownload,
    Globe01,
    HelpCircle,
    InfoCircle,
    Key01,
    LayoutTop,
    Link03,
    LinkExternal01,
    Loading02,
    Maximize01,
    MessageTextSquare01,
    Minus,
    Modrinth,
    OnboardingAccount,
    OnboardingComplete,
    OnboardingLanguage,
    OnboardingPreferences,
    OnboardingWelcome,
    PaintPour,
    ParagraphWrap,
    Pencil01,
    Play,
    Plus,
    RefreshCcw02,
    RefreshCw01,
    Rocket02,
    SearchMd,
    Settings01,
    Settings02,
    Settings04,
    Sliders04,
    Square,
    Terminal,
    Trash01,
    Users01,
    XClose,
    X,
}
