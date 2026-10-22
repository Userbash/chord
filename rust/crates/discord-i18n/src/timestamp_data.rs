//! Сгенерированные CLDR-паттерны Discord-таймстемпов `<t:…>` (P7-01e).
//!
//! Источник: Node v24.21.0 / ICU 78.3, `Intl.DateTimeFormat`
//! с `timeZone: "UTC"`, скрипт `waves/RW-29/gen-timestamp-data.mjs`.
//! **Ручная правка запрещена.** 31 включённая локаль × 6 стилей (`t T d D f F`) = 186.

/// Данные одной локали: паттерны в порядке `t T d D f F` и длинные названия дней
/// недели в контексте даты (в финском эссив `lauantaina`, а не `lauantai`).
pub(crate) struct TimestampLocaleData {
    pub locale: &'static str,
    pub patterns: [&'static str; 6],
    pub weekdays_long: [&'static str; 7],
}

#[rustfmt::skip]
pub(crate) static TIMESTAMP_LOCALES: &[TimestampLocaleData] = &[
    TimestampLocaleData {
        locale: "bg",
        patterns: ["H':'mm", "H':'mm':'ss", "dd'.'MM'.'y' г.'", "d' 'MMMM' 'y' г.'", "dd'.'MM'.'y' г., 'H':'mm", "EEEE', 'd' 'MMMM' 'y' г. в 'H':'mm"],
        weekdays_long: ["неделя", "понеделник", "вторник", "сряда", "четвъртък", "петък", "събота"],
    },
    TimestampLocaleData {
        locale: "cs",
        patterns: ["H':'mm", "H':'mm':'ss", "dd'. 'MM'. 'y", "d'. 'MMMM' 'y", "dd'. 'MM'. 'y' 'H':'mm", "EEEE' 'd'. 'MMMM' 'y' v 'H':'mm"],
        weekdays_long: ["neděle", "pondělí", "úterý", "středa", "čtvrtek", "pátek", "sobota"],
    },
    TimestampLocaleData {
        locale: "da",
        patterns: ["H'.'mm", "H'.'mm'.'ss", "dd'.'MM'.'y", "d'. 'MMMM' 'y", "dd'.'MM'.'y', 'H'.'mm", "EEEE' den 'd'. 'MMMM' 'y' kl. 'H'.'mm"],
        weekdays_long: ["søndag", "mandag", "tirsdag", "onsdag", "torsdag", "fredag", "lørdag"],
    },
    TimestampLocaleData {
        locale: "de",
        patterns: ["H':'mm", "H':'mm':'ss", "dd'.'MM'.'y", "d'. 'MMMM' 'y", "dd'.'MM'.'y', 'H':'mm", "EEEE', 'd'. 'MMMM' 'y' um 'H':'mm"],
        weekdays_long: ["Sonntag", "Montag", "Dienstag", "Mittwoch", "Donnerstag", "Freitag", "Samstag"],
    },
    TimestampLocaleData {
        locale: "el",
        patterns: ["h':'mm' 'a", "h':'mm':'ss' 'a", "dd'/'MM'/'y", "d' 'MMMM' 'y", "dd'/'MM'/'y', 'h':'mm' 'a", "EEEE' 'd' 'MMMM' 'y' στις 'h':'mm' 'a"],
        weekdays_long: ["Κυριακή", "Δευτέρα", "Τρίτη", "Τετάρτη", "Πέμπτη", "Παρασκευή", "Σάββατο"],
    },
    TimestampLocaleData {
        locale: "en-GB",
        patterns: ["H':'mm", "H':'mm':'ss", "dd'/'MM'/'y", "d' 'MMMM' 'y", "dd'/'MM'/'y', 'H':'mm", "EEEE', 'd' 'MMMM' 'y' at 'H':'mm"],
        weekdays_long: ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"],
    },
    TimestampLocaleData {
        locale: "en-US",
        patterns: ["h':'mm' 'a", "h':'mm':'ss' 'a", "MM'/'dd'/'y", "MMMM' 'd', 'y", "MM'/'dd'/'y', 'h':'mm' 'a", "EEEE', 'MMMM' 'd', 'y' at 'h':'mm' 'a"],
        weekdays_long: ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"],
    },
    TimestampLocaleData {
        locale: "es-419",
        patterns: ["h':'mm' 'a", "h':'mm':'ss' 'a", "dd'/'MM'/'y", "d' de 'MMMM' de 'y", "dd'/'MM'/'y', 'h':'mm' 'a", "EEEE', 'd' de 'MMMM' de 'y', 'h':'mm' 'a"],
        weekdays_long: ["domingo", "lunes", "martes", "miércoles", "jueves", "viernes", "sábado"],
    },
    TimestampLocaleData {
        locale: "es-ES",
        patterns: ["H':'mm", "H':'mm':'ss", "dd'/'MM'/'y", "d' de 'MMMM' de 'y", "dd'/'MM'/'y', 'H':'mm", "EEEE', 'd' de 'MMMM' de 'y', 'H':'mm"],
        weekdays_long: ["domingo", "lunes", "martes", "miércoles", "jueves", "viernes", "sábado"],
    },
    TimestampLocaleData {
        locale: "fi",
        patterns: ["H'.'mm", "H'.'mm'.'ss", "dd'.'MM'.'y", "d'. 'MMMM' 'y", "dd'.'MM'.'y' klo 'H'.'mm", "EEEE' 'd'. 'MMMM' 'y' klo 'H'.'mm"],
        weekdays_long: ["sunnuntaina", "maanantaina", "tiistaina", "keskiviikkona", "torstaina", "perjantaina", "lauantaina"],
    },
    TimestampLocaleData {
        locale: "fr",
        patterns: ["H':'mm", "H':'mm':'ss", "dd'/'MM'/'y", "d' 'MMMM' 'y", "dd'/'MM'/'y' 'H':'mm", "EEEE' 'd' 'MMMM' 'y' à 'H':'mm"],
        weekdays_long: ["dimanche", "lundi", "mardi", "mercredi", "jeudi", "vendredi", "samedi"],
    },
    TimestampLocaleData {
        locale: "hi",
        patterns: ["h':'mm' 'a", "h':'mm':'ss' 'a", "dd'/'MM'/'y", "d' 'MMMM' 'y", "dd'/'MM'/'y', 'h':'mm' 'a", "EEEE', 'd' 'MMMM' 'y' को 'h':'mm' 'a' बजे'"],
        weekdays_long: ["रविवार", "सोमवार", "मंगलवार", "बुधवार", "गुरुवार", "शुक्रवार", "शनिवार"],
    },
    TimestampLocaleData {
        locale: "hr",
        patterns: ["H':'mm", "H':'mm':'ss", "dd'. 'MM'. 'y'.'", "d'. 'MMMM' 'y'.'", "dd'. 'MM'. 'y'. 'H':'mm", "EEEE', 'd'. 'MMMM' 'y'. u 'H':'mm"],
        weekdays_long: ["nedjelja", "ponedjeljak", "utorak", "srijeda", "četvrtak", "petak", "subota"],
    },
    TimestampLocaleData {
        locale: "hu",
        patterns: ["H':'mm", "H':'mm':'ss", "y'. 'MM'. 'dd'.'", "y'. 'MMMM' 'd'.'", "y'. 'MM'. 'dd'. 'H':'mm", "y'. 'MMMM' 'd'., 'EEEE' 'H':'mm"],
        weekdays_long: ["vasárnap", "hétfő", "kedd", "szerda", "csütörtök", "péntek", "szombat"],
    },
    TimestampLocaleData {
        locale: "it",
        patterns: ["H':'mm", "H':'mm':'ss", "dd'/'MM'/'y", "d' 'MMMM' 'y", "dd'/'MM'/'y', 'H':'mm", "EEEE' 'd' 'MMMM' 'y' alle ore 'H':'mm"],
        weekdays_long: ["domenica", "lunedì", "martedì", "mercoledì", "giovedì", "venerdì", "sabato"],
    },
    TimestampLocaleData {
        locale: "ja",
        patterns: ["H':'mm", "H':'mm':'ss", "y'/'MM'/'dd", "y'年'MMMM'月'd'日'", "y'/'MM'/'dd' 'H':'mm", "y'年'MMMM'月'd'日'EEEE' 'H':'mm"],
        weekdays_long: ["日曜日", "月曜日", "火曜日", "水曜日", "木曜日", "金曜日", "土曜日"],
    },
    TimestampLocaleData {
        locale: "ko",
        patterns: ["a' 'h':'mm", "a' 'h':'mm':'ss", "y'. 'MM'. 'dd'.'", "y'년 'MMMM' 'd'일'", "y'. 'MM'. 'dd'. 'a' 'h':'mm", "y'년 'MMMM' 'd'일 'EEEE' 'a' 'h':'mm"],
        weekdays_long: ["일요일", "월요일", "화요일", "수요일", "목요일", "금요일", "토요일"],
    },
    TimestampLocaleData {
        locale: "lt",
        patterns: ["H':'mm", "H':'mm':'ss", "y'-'MM'-'dd", "y' m. 'MMMM' 'd' d.'", "y'-'MM'-'dd' 'H':'mm", "y' m. 'MMMM' 'd' d., 'EEEE' 'H':'mm"],
        weekdays_long: ["sekmadienis", "pirmadienis", "antradienis", "trečiadienis", "ketvirtadienis", "penktadienis", "šeštadienis"],
    },
    TimestampLocaleData {
        locale: "nl",
        patterns: ["H':'mm", "H':'mm':'ss", "dd'-'MM'-'y", "d' 'MMMM' 'y", "dd'-'MM'-'y', 'H':'mm", "EEEE' 'd' 'MMMM' 'y' om 'H':'mm"],
        weekdays_long: ["zondag", "maandag", "dinsdag", "woensdag", "donderdag", "vrijdag", "zaterdag"],
    },
    TimestampLocaleData {
        locale: "no",
        patterns: ["H':'mm", "H':'mm':'ss", "dd'.'MM'.'y", "d'. 'MMMM' 'y", "dd'.'MM'.'y', 'H':'mm", "EEEE' 'd'. 'MMMM' 'y' kl. 'H':'mm"],
        weekdays_long: ["søndag", "mandag", "tirsdag", "onsdag", "torsdag", "fredag", "lørdag"],
    },
    TimestampLocaleData {
        locale: "pl",
        patterns: ["H':'mm", "H':'mm':'ss", "dd'.'MM'.'y", "d' 'MMMM' 'y", "dd'.'MM'.'y', 'H':'mm", "EEEE', 'd' 'MMMM' 'y' 'H':'mm"],
        weekdays_long: ["niedziela", "poniedziałek", "wtorek", "środa", "czwartek", "piątek", "sobota"],
    },
    TimestampLocaleData {
        locale: "pt-BR",
        patterns: ["H':'mm", "H':'mm':'ss", "dd'/'MM'/'y", "d' de 'MMMM' de 'y", "dd'/'MM'/'y', 'H':'mm", "EEEE', 'd' de 'MMMM' de 'y' às 'H':'mm"],
        weekdays_long: ["domingo", "segunda-feira", "terça-feira", "quarta-feira", "quinta-feira", "sexta-feira", "sábado"],
    },
    TimestampLocaleData {
        locale: "ro",
        patterns: ["H':'mm", "H':'mm':'ss", "dd'.'MM'.'y", "d' 'MMMM' 'y", "dd'.'MM'.'y', 'H':'mm", "EEEE', 'd' 'MMMM' 'y' la 'H':'mm"],
        weekdays_long: ["duminică", "luni", "marți", "miercuri", "joi", "vineri", "sâmbătă"],
    },
    TimestampLocaleData {
        locale: "ru",
        patterns: ["H':'mm", "H':'mm':'ss", "dd'.'MM'.'y", "d' 'MMMM' 'y' г.'", "dd'.'MM'.'y', 'H':'mm", "EEEE', 'd' 'MMMM' 'y' г. в 'H':'mm"],
        weekdays_long: ["воскресенье", "понедельник", "вторник", "среда", "четверг", "пятница", "суббота"],
    },
    TimestampLocaleData {
        locale: "sv-SE",
        patterns: ["H':'mm", "H':'mm':'ss", "y'-'MM'-'dd", "d' 'MMMM' 'y", "y'-'MM'-'dd' 'H':'mm", "EEEE' 'd' 'MMMM' 'y' kl. 'H':'mm"],
        weekdays_long: ["söndag", "måndag", "tisdag", "onsdag", "torsdag", "fredag", "lördag"],
    },
    TimestampLocaleData {
        locale: "th",
        patterns: ["H':'mm", "H':'mm':'ss", "dd'/'MM'/'y", "d' 'MMMM' 'y", "dd'/'MM'/'y' 'H':'mm", "EEEE'ที่ 'd' 'MMMM' 'y' เวลา 'H':'mm"],
        weekdays_long: ["วันอาทิตย์", "วันจันทร์", "วันอังคาร", "วันพุธ", "วันพฤหัสบดี", "วันศุกร์", "วันเสาร์"],
    },
    TimestampLocaleData {
        locale: "tr",
        patterns: ["H':'mm", "H':'mm':'ss", "dd'.'MM'.'y", "d' 'MMMM' 'y", "dd'.'MM'.'y' 'H':'mm", "d' 'MMMM' 'y' 'EEEE' 'H':'mm"],
        weekdays_long: ["Pazar", "Pazartesi", "Salı", "Çarşamba", "Perşembe", "Cuma", "Cumartesi"],
    },
    TimestampLocaleData {
        locale: "uk",
        patterns: ["H':'mm", "H':'mm':'ss", "dd'.'MM'.'y", "d' 'MMMM' 'y' р.'", "dd'.'MM'.'y', 'H':'mm", "EEEE', 'd' 'MMMM' 'y' р. о 'H':'mm"],
        weekdays_long: ["неділя", "понеділок", "вівторок", "середа", "четвер", "пʼятниця", "субота"],
    },
    TimestampLocaleData {
        locale: "vi",
        patterns: ["H':'mm", "H':'mm':'ss", "dd'/'MM'/'y", "d' 'MMMM', 'y", "H':'mm' 'dd'/'MM'/'y", "'lúc 'H':'mm' 'EEEE', 'd' 'MMMM', 'y"],
        weekdays_long: ["Chủ Nhật", "Thứ Hai", "Thứ Ba", "Thứ Tư", "Thứ Năm", "Thứ Sáu", "Thứ Bảy"],
    },
    TimestampLocaleData {
        locale: "zh-CN",
        patterns: ["H':'mm", "H':'mm':'ss", "y'/'MM'/'dd", "y'年'MMMM'月'd'日'", "y'/'MM'/'dd' 'H':'mm", "y'年'MMMM'月'd'日'EEEE' 'H':'mm"],
        weekdays_long: ["星期日", "星期一", "星期二", "星期三", "星期四", "星期五", "星期六"],
    },
    TimestampLocaleData {
        locale: "zh-TW",
        patterns: ["ah':'mm", "ah':'mm':'ss", "y'/'MM'/'dd", "y'年'MMMM'月'd'日'", "y'/'MM'/'dd' 'ah':'mm", "y'年'MMMM'月'd'日'EEEE' 'ah':'mm"],
        weekdays_long: ["星期日", "星期一", "星期二", "星期三", "星期四", "星期五", "星期六"],
    },
];
