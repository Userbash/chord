//! Детерминированный джиттер-буфер RTP (P4-04).
//!
//! Окно — по модулю 2^16: «впереди» значит `sequence.wrapping_sub(start) < 32768`.
//! Буфер не работает со временем и сетью: порядок, дубли и потери задаются
//! последовательностью `push`/`pop`, поэтому контракт проверяется без sleep и
//! live Discord.
//!
//! Трактовка §1 ТЗ: «первый push задаёт ожидаемый seq» — это старт окна. Пока не
//! было ни одной выдачи, пакет позади старта ещё не «отдан и не пропущен», поэтому
//! он не `Late`, а расширяет окно назад (реордер 3→1→2 выдаётся как 1,2,3).
//! После первой выдачи граница окна совпадает с фронтом доставки, и `seq` позади
//! неё — `Late` (пропущен) либо `Duplicate` (если он уже был отдан).

use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// Половина пространства sequence: граница «впереди/позади».
const SEQUENCE_WINDOW: u16 = 32_768;

/// Сколько последних выданных seq помним для детекта дублей (10 с аудио).
const DELIVERED_HISTORY: usize = 512;

/// Счётчики буфера за время жизни.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct JitterStats {
    /// Принято в буфер (`PushOutcome::Queued`).
    pub received: u64,
    /// Отброшено как дубль (уже в буфере или уже отдан).
    pub duplicates: u64,
    /// Отброшено как опоздавшее (позади фронта и не было отдано).
    pub late: u64,
    /// Сколько seq пропущено (по одному на `PopOutcome::Missing`).
    pub missing: u64,
    /// Отброшено из-за заполненного буфера.
    pub overflow: u64,
}

/// Результат `push`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PushOutcome {
    /// Пакет принят в буфер.
    Queued,
    /// Такой seq уже в буфере или уже был отдан.
    Duplicate,
    /// Seq позади окна и не был отдан (пропущен).
    Late,
    /// Буфер полон, пакет не принят.
    Overflow,
}

/// Результат `pop`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PopOutcome {
    /// Очередной пакет в порядке sequence.
    Frame { sequence: u16, payload: Vec<u8> },
    /// Ожидаемый seq отсутствует: зафиксирован пропуск.
    Missing { sequence: u16 },
    /// Буфер пуст.
    Empty,
}

/// Джиттер-буфер с окном по модулю 2^16.
#[derive(Debug, Clone)]
pub struct JitterBuffer {
    slots: BTreeMap<u16, Vec<u8>>,
    window_start: Option<u16>,
    started: bool,
    delivered: BTreeSet<u16>,
    delivered_order: VecDeque<u16>,
    capacity: usize,
    stats: JitterStats,
}

impl JitterBuffer {
    /// Новый буфер; `capacity == 0` трактуется как 1.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            slots: BTreeMap::new(),
            window_start: None,
            started: false,
            delivered: BTreeSet::new(),
            delivered_order: VecDeque::new(),
            capacity: capacity.max(1),
            stats: JitterStats::default(),
        }
    }

    /// Ёмкость буфера (после нормализации нуля).
    #[must_use]
    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Сколько пакетов сейчас лежит в буфере.
    #[must_use]
    pub fn buffered(&self) -> usize {
        self.slots.len()
    }

    /// Снимок счётчиков.
    #[must_use]
    pub fn stats(&self) -> JitterStats {
        self.stats
    }

    /// Принимает пакет.
    ///
    /// Порядок проверок: дубль (в буфере или уже отдан) → опоздавший (позади
    /// фронта, если выдача уже началась) → переполнение → приём. Пока выдача не
    /// началась, пакет позади старта окна расширяет окно назад.
    pub fn push(&mut self, sequence: u16, payload: Vec<u8>) -> PushOutcome {
        if self.slots.contains_key(&sequence) || self.delivered.contains(&sequence) {
            self.stats.duplicates += 1;
            return PushOutcome::Duplicate;
        }
        if let Some(start) = self.window_start
            && self.started
            && is_behind(sequence, start)
        {
            self.stats.late += 1;
            return PushOutcome::Late;
        }
        if self.slots.len() >= self.capacity {
            self.stats.overflow += 1;
            return PushOutcome::Overflow;
        }
        match self.window_start {
            // Окно ещё не началось: этот seq становится ожидаемым.
            None => self.window_start = Some(sequence),
            // Выдача не начиналась: реордер расширяет окно назад.
            Some(start) if is_behind(sequence, start) => self.window_start = Some(sequence),
            Some(_) => {}
        }
        self.slots.insert(sequence, payload);
        self.stats.received += 1;
        PushOutcome::Queued
    }

    /// Отдаёт следующий пакет или фиксирует ровно один пропуск.
    ///
    /// Ожидаемый seq отдаётся и окно сдвигается на 1. Если ожидаемого нет, но
    /// впереди есть пакеты, возвращается `Missing` (счётчик `missing += 1`), и
    /// следующий `pop` продолжит с этого места. Пустой буфер — `Empty`.
    pub fn pop(&mut self) -> PopOutcome {
        let Some(start) = self.window_start else {
            return PopOutcome::Empty;
        };
        if self.slots.is_empty() {
            return PopOutcome::Empty;
        }
        if let Some(payload) = self.slots.remove(&start) {
            self.window_start = Some(start.wrapping_add(1));
            self.started = true;
            self.record_delivered(start);
            return PopOutcome::Frame {
                sequence: start,
                payload,
            };
        }
        if self
            .slots
            .keys()
            .any(|sequence| !is_behind(*sequence, start))
        {
            self.window_start = Some(start.wrapping_add(1));
            self.started = true;
            self.stats.missing += 1;
            return PopOutcome::Missing { sequence: start };
        }
        // Инвариант: при непустом буфере и started позади фронта пакетов не бывает
        // (push отбрасывает их как Late, до старта окно расширяется назад).
        // Ветка — защитная, без паники.
        PopOutcome::Empty
    }

    /// Запоминает выданный seq для детекта дублей с ограниченной историей.
    fn record_delivered(&mut self, sequence: u16) {
        if self.delivered.insert(sequence) {
            self.delivered_order.push_back(sequence);
        }
        while self.delivered_order.len() > DELIVERED_HISTORY {
            if let Some(oldest) = self.delivered_order.pop_front() {
                self.delivered.remove(&oldest);
            }
        }
    }
}

/// `true`, если `sequence` позади `start` в окне по модулю 2^16.
fn is_behind(sequence: u16, start: u16) -> bool {
    sequence.wrapping_sub(start) >= SEQUENCE_WINDOW
}
