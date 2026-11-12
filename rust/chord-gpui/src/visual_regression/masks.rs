use serde::{Deserialize, Serialize};

use super::VisualRegressionError;

/// Категории динамических зон интерфейса, подлежащих маскированию при расчёте SSIM
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MaskCategory {
    Clock,
    FpsCounter,
    Avatar,
    PingIndicator,
}

/// Прямоугольная маска динамической области в пикселях координат экрана
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MaskRegion {
    pub id: String,
    pub category: MaskCategory,
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub description: String,
}

impl MaskRegion {
    pub fn contains(&self, px: u32, py: u32) -> bool {
        let x_end = self.x.saturating_add(self.width);
        let y_end = self.y.saturating_add(self.height);
        px >= self.x && px < x_end && py >= self.y && py < y_end
    }

    pub fn is_border(&self, px: u32, py: u32) -> bool {
        if !self.contains(px, py) {
            return false;
        }
        let x_end = self.x.saturating_add(self.width).saturating_sub(1);
        let y_end = self.y.saturating_add(self.height).saturating_sub(1);
        px == self.x || px == x_end || py == self.y || py == y_end
    }

    pub fn fits_within(&self, viewport_w: u32, viewport_h: u32) -> bool {
        self.width > 0
            && self.height > 0
            && self.x.saturating_add(self.width) <= viewport_w
            && self.y.saturating_add(self.height) <= viewport_h
    }
}

/// Набор JSON-масок динамических зон для заданного разрешения экрана
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DynamicMaskSet {
    pub viewport_width: u32,
    pub viewport_height: u32,
    pub regions: Vec<MaskRegion>,
}

impl DynamicMaskSet {
    pub fn default_for_resolution(width: u32, height: u32) -> Self {
        let regions = vec![
            Self::build_clock_header_mask(width),
            Self::build_fps_badge_mask(width),
            Self::build_chat_timestamps_mask(height),
            Self::build_chat_avatars_mask(height),
            Self::build_user_panel_avatar_mask(height),
            Self::build_voice_ping_mask(height),
            Self::build_member_avatars_mask(width, height),
        ];
        Self {
            viewport_width: width,
            viewport_height: height,
            regions,
        }
    }

    fn build_clock_header_mask(width: u32) -> MaskRegion {
        MaskRegion {
            id: "header-system-clock".into(),
            category: MaskCategory::Clock,
            x: width.saturating_sub(500),
            y: 10,
            width: 100,
            height: 28,
            description: "Системные часы в верхней панели заголовка чата".into(),
        }
    }

    fn build_fps_badge_mask(width: u32) -> MaskRegion {
        MaskRegion {
            id: "header-fps-counter".into(),
            category: MaskCategory::FpsCounter,
            x: width.saturating_sub(380),
            y: 10,
            width: 120,
            height: 28,
            description: "Индикатор частоты кадров 120 FPS и времени кадра".into(),
        }
    }

    fn build_chat_timestamps_mask(height: u32) -> MaskRegion {
        MaskRegion {
            id: "chat-message-timestamps".into(),
            category: MaskCategory::Clock,
            x: 430,
            y: 64,
            width: 110,
            height: height.saturating_sub(140).max(60),
            description: "Колонка временных меток сообщений в ленте чата".into(),
        }
    }

    fn build_chat_avatars_mask(height: u32) -> MaskRegion {
        MaskRegion {
            id: "chat-author-avatars".into(),
            category: MaskCategory::Avatar,
            x: 324,
            y: 64,
            width: 48,
            height: height.saturating_sub(140).max(60),
            description: "Круглые 40x40 аватарки авторов сообщений в ленте".into(),
        }
    }

    fn build_user_panel_avatar_mask(height: u32) -> MaskRegion {
        MaskRegion {
            id: "user-panel-avatar".into(),
            category: MaskCategory::Avatar,
            x: 80,
            y: height.saturating_sub(46),
            width: 36,
            height: 36,
            description: "Аватар текущего пользователя в нижней плашке профиля".into(),
        }
    }

    fn build_voice_ping_mask(height: u32) -> MaskRegion {
        MaskRegion {
            id: "voice-rtc-ping-badge".into(),
            category: MaskCategory::PingIndicator,
            x: 80,
            y: height.saturating_sub(98),
            width: 220,
            height: 44,
            description: "Индикатор RTC пинга и качества голосовой связи".into(),
        }
    }

    fn build_member_avatars_mask(width: u32, height: u32) -> MaskRegion {
        MaskRegion {
            id: "member-sidebar-avatars".into(),
            category: MaskCategory::Avatar,
            x: width.saturating_sub(232),
            y: 64,
            width: 40,
            height: height.saturating_sub(80).max(60),
            description: "Аватарки и статус-индикаторы участников в правом сайдбаре".into(),
        }
    }

    pub fn validate(&self, width: u32, height: u32) -> Result<(), VisualRegressionError> {
        if self.viewport_width != width || self.viewport_height != height {
            return Err(VisualRegressionError::DimensionMismatch {
                expected: (width, height),
                actual: (self.viewport_width, self.viewport_height),
            });
        }
        for region in &self.regions {
            if !region.fits_within(width, height) {
                return Err(VisualRegressionError::MaskOutOfBounds {
                    mask_id: region.id.clone(),
                    rect: (region.x, region.y, region.width, region.height),
                    frame: (width, height),
                });
            }
        }
        Ok(())
    }

    pub fn is_masked(&self, x: u32, y: u32) -> bool {
        self.regions.iter().any(|r| r.contains(x, y))
    }

    pub fn to_bitmap(&self) -> Vec<bool> {
        let w = self.viewport_width as usize;
        let h = self.viewport_height as usize;
        let mut bitmap = vec![false; w * h];
        for region in &self.regions {
            let x0 = (region.x as usize).min(w);
            let x1 = (region.x.saturating_add(region.width) as usize).min(w);
            let y0 = (region.y as usize).min(h);
            let y1 = (region.y.saturating_add(region.height) as usize).min(h);
            for y in y0..y1 {
                let row_start = y * w;
                bitmap[row_start + x0..row_start + x1].fill(true);
            }
        }
        bitmap
    }

    pub fn from_json(json: &str) -> Result<Self, VisualRegressionError> {
        serde_json::from_str(json).map_err(|e| VisualRegressionError::JsonError(e.to_string()))
    }

    pub fn to_json_pretty(&self) -> Result<String, VisualRegressionError> {
        serde_json::to_string_pretty(self)
            .map_err(|e| VisualRegressionError::JsonError(e.to_string()))
    }
}
