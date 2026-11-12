//! Модуль превью и кеширования медиа-вложений (PNG/JPEG)
//! Использует стандартный крейт `image` вместо самописных декодеров.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Метаданные изображения-вложения
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ImageAttachmentMeta {
    pub id: String,
    pub filename: String,
    pub width: u32,
    pub height: u32,
    pub size_bytes: usize,
    pub content_type: String,
}

impl ImageAttachmentMeta {
    /// Расчёт пропорциональных размеров для превью в чате
    pub fn compute_preview_dimensions(&self, max_w: u32, max_h: u32) -> (u32, u32) {
        if self.width == 0 || self.height == 0 {
            return (max_w, max_h);
        }

        let width_ratio = max_w as f64 / self.width as f64;
        let height_ratio = max_h as f64 / self.height as f64;
        let scale = width_ratio.min(height_ratio).min(1.0);

        let final_w = (self.width as f64 * scale).round() as u32;
        let final_h = (self.height as f64 * scale).round() as u32;

        (final_w.max(1), final_h.max(1))
    }
}

/// Кеш медиа-вложений в оперативной памяти
#[derive(Clone, Default)]
pub struct MediaCache {
    entries: Arc<Mutex<HashMap<String, Vec<u8>>>>,
}

impl MediaCache {
    pub fn new() -> Self {
        Self {
            entries: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Сохранить байты изображения в кеш
    pub fn put(&self, key: &str, data: Vec<u8>) {
        if let Ok(mut lock) = self.entries.lock() {
            lock.insert(key.to_string(), data);
        }
    }

    /// Получить байты изображения из кеша
    pub fn get(&self, key: &str) -> Option<Vec<u8>> {
        self.entries.lock().ok()?.get(key).cloned()
    }

    /// Количество элементов в кеше
    pub fn len(&self) -> usize {
        self.entries.lock().map(|l| l.len()).unwrap_or(0)
    }

    /// Декодирование размеров изображения через стандартный крейт `image`
    pub fn inspect_image_dimensions(bytes: &[u8]) -> Result<(u32, u32), String> {
        let reader = image::ImageReader::new(std::io::Cursor::new(bytes))
            .with_guessed_format()
            .map_err(|e| format!("Ошибка определения формата: {e}"))?;

        let (w, h) = reader
            .into_dimensions()
            .map_err(|e| format!("Ошибка чтения размеров изображения: {e}"))?;

        Ok((w, h))
    }
}
