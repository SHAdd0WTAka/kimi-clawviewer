//! Utility functions for the ClawViewer project.
//!
//! Provides password generation (human-friendly session passwords) and
//! timestamp helpers for cross-platform use.

use rand::seq::SliceRandom;
use rand::thread_rng;

use crate::types::Password;

// ---------------------------------------------------------------------------
// Password generation
// ---------------------------------------------------------------------------

/// A built-in wordlist of exactly 6-character uppercase English words.
///
/// These words are chosen to be easy to read, spell, and type over the phone
/// or in a chat message. Confusing letters (O, I, L) are excluded to avoid
/// ambiguity with digits 0 and 1.
const PASSWORD_WORDLIST: &[&str] = &[
    "ABDUCT", "ABJECT", "ABRUPT", "ABSENT", "ABSURD", "ACCENT", "ACCEPT", "ACCESS",
    "ACCUSE", "ADAPTS", "ADJUST", "ADVENT", "ADVERB", "AFFECT", "AGENDA", "AGENTS",
    "AGREED", "AMAZED", "AMBUSH", "AMENDS", "AMUSED", "ANSWER", "ANTHEM", "ANYWAY",
    "APPEAR", "APPEND", "ARCADE", "ARCHER", "ARDENT", "ARGUED", "ARMADA", "ARREST",
    "ARTERY", "ASCEND", "ASPECT", "ASSENT", "ASSERT", "ASSETS", "ASSUME", "ASSURE",
    "ASTERN", "ASTUTE", "ATTACH", "ATTACK", "ATTEND", "AUBURN", "AUGUST", "AUTUMN",
    "AVENUE", "AVERSE", "AWAKEN", "AWARDS", "BANTER", "BARGES", "BASSET", "BASTER",
    "BATHES", "BATMAN", "BEASTS", "BEATEN", "BEAUTY", "BEGGAR", "BEHAVE", "BEHEAD",
    "BETRAY", "BETTER", "BEWARE", "BRANCH", "BRANDS", "BREACH", "BREAST", "BREEZE",
    "BRUNCH", "BUCKET", "BUDGET", "BUFFET", "BUMPER", "BUNYAN", "BURDEN", "BUREAU",
    "BURSTS", "BUSHES", "BUTTER", "BUYERS", "BUZZED", "CADETS", "CAESAR", "CAGERS",
    "CAMBER", "CAMERA", "CAMPED", "CAMPUS", "CANCER", "CANNED", "CANVAS", "CAPERS",
    "CARATS", "CAREER", "CARPET", "CARTER", "CARVED", "CASHEW", "CASKET", "CATNAP",
    "CAUGHT", "CAUSED", "CAVEAT", "CAVERN", "CEASED", "CEDARS", "CENSUS", "CENTER",
    "CHANCE", "CHANGE", "CHANTS", "CHARGE", "CHASED", "CHASTE", "CHEATS", "CHECKS",
    "CHEERS", "CHEESE", "CHERRY", "CHUNKS", "CHURCH", "CRACKS", "CRAFTY", "CRATER",
    "CREAKY", "CREEPS", "CRUNCH", "CRUSTY", "CUPPED", "CURBED", "CURFEW", "CURSED",
    "CURVES", "DAMAGE", "DANCER", "DANGER", "DARTED", "DAWNED", "DEADEN", "DEAFEN",
    "DEARER", "DEBATE", "DECADE", "DECENT", "DECREE", "DEDUCT", "DEEMED", "DEEPEN",
    "DEFACE", "DEFEAT", "DEFECT", "DEFEND", "DEFUSE", "DEGREE", "DEMAND", "DEMEAN",
    "DEMURE", "DENSER", "DEPART", "DEPEND", "DEPUTY", "DESERT", "DETACH", "DETECT",
    "DRAFTS", "DRAMAS", "DRAPED", "DRAWER", "DREADS", "DREAMS", "DREAMT", "DREARY",
    "DRENCH", "DRESSY", "DUBBED", "DUMPED", "DUSTED", "EARTHY", "EASTER", "EATERS",
    "EFFACE", "EFFECT", "EGGERS", "EGRETS", "EMBARK", "EMERGE", "ENCAMP", "ENDEAR",
    "ENDURE", "ENERGY", "ENGAGE", "ENSURE", "ENTERS", "ENZYME", "EQUATE", "ERASED",
    "ERECTS", "ERUPTS", "ESCAPE", "ESTATE", "ESTEEM", "EVADED", "EVENTS", "EXCEED",
    "EXCUSE", "EXEMPT", "EXERTS", "EXPAND", "EXPECT", "EXPERT", "EXTEND", "EXTENT",
    "FACADE", "FACETS", "FAKERS", "FARMER", "FASTEN", "FATHER", "FATTEN", "FAUCET",
    "FEARED", "FEASTS", "FECUND", "FEEDER", "FENDER", "FERRET", "FESTER", "FETTER",
    "FRAMES", "FRANKS", "FRAYED", "FREAKS", "FREEZE", "FRENZY", "FUDGED", "FUGUES",
    "FUNDED", "FUNGUS", "FURRED", "FUSSED", "FUTURE", "GAGGED", "GANDER", "GARAGE",
    "GARDEN", "GARNET", "GARTER", "GASHED", "GATHER", "GAUCHE", "GAUGED", "GAZERS",
    "GEARED", "GENDER", "GENRES", "GENTRY", "GEYSER", "GNAWED", "GRACED", "GRACES",
    "GRADES", "GRAFTS", "GRAMME", "GRANDS", "GRANGE", "GRANTS", "GRAPES", "GRASSY",
    "GRATED", "GRATER", "GRAVEN", "GRAVES", "GRAZED", "GREASE", "GREASY", "GREEDY",
    "GRUDGE", "GRUMPY", "GUARDS", "GUESTS", "GUNMEN", "GUNNER", "GURNEY", "GUSHED",
    "GUTTER", "GYPSUM", "GYRATE", "HACKED", "HACKER", "HANDED", "HANGAR", "HANGED",
    "HANGER", "HAPPEN", "HARDEN", "HASTEN", "HATRED", "HAUNTS", "HAVENS", "HAWKER",
    "HAZARD", "HEADED", "HEADER", "HEARSE", "HEARTH", "HEARTS", "HEARTY", "HEATED",
    "HEATER", "HEAVED", "HEAVEN", "HEDGED", "HEFTED", "HEREAT", "HERESY", "HUBBUB",
    "HUFFED", "HUGGED", "HUMANE", "HUMMED", "HUMPED", "HUNGER", "HUNGRY", "HUNTED",
    "HUNTER", "HURRAH", "HURRAY", "HUSKED", "HYMENS", "JACKED", "JACKET", "JAGGED",
    "JAGUAR", "JAMMED", "JAUNTY", "JESTER", "JETSAM", "JUDGED", "JUMPED", "JUMPER",
    "JUNKET", "KEEPER", "KERBED", "KEYPAD", "KNACKS", "KNEADS", "MACAWS", "MADAME",
    "MADDEN", "MAGNET", "MAKEUP", "MANAGE", "MANNER", "MANTRA", "MAPPED", "MARKED",
    "MARKER", "MARKET", "MARRED", "MARSHY", "MARTEN", "MASHER", "MASKED", "MASTED",
    "MASTER", "MATTER", "MATURE", "MAYDAY", "MAYHEM", "MEAGER", "MEANER", "MEMBER",
    "MENACE", "MENDED", "MERGED", "MERGER", "MERMEN", "MESHED", "MUFFED", "MUGGED",
    "MURDER", "MURMUR", "MUSEUM", "MUSHED", "MUSKET", "MUSSED", "MUSTER", "MUTANT",
    "MUTTER", "NAGGED", "NAPPED", "NATURE", "NAUGHT", "NEARBY", "NEATER", "NECTAR",
    "NEEDED", "NEGATE", "NEPHEW", "NERVES", "NESTED", "NETTED", "NEUTER", "NEWEST",
    "NUANCE", "NUDGED", "NUMBER", "NURSED", "NUTMEG", "NYMPHS", "PACKED", "PACKER",
    "PACKET", "PADDED", "PAGERS", "PAMPER", "PANDAS", "PANTRY", "PAPERS", "PARADE",
    "PARENT", "PARKED", "PARSED", "PARTED", "PASSED", "PASTED", "PATCHY", "PATENT",
    "PATTED", "PAUSED", "PECKED", "PEDANT", "PEEKED", "PEEPED", "PEERED", "PENNED",
    "PEPPER", "PERUSE", "PESTER", "PETTED", "PHASED", "PHRASE", "PRANCE", "PRANKS",
    "PRAYER", "PREACH", "PREFER", "PREPAY", "PRESET", "PRETTY", "PREVUE", "PRUNED",
    "PRUNES", "PSYCHE", "PUCKER", "PUFFED", "PUMPED", "PUNNED", "PUPPET", "PUREST",
    "PURGED", "PURSED", "PURSUE", "PUSHED", "QUACKS", "QUAKED", "QUARRY", "QUARTZ",
    "QUASAR", "QUAVER", "QUEASY", "QUEENS", "QUENCH", "QUESTS", "QUEUED", "RACERS",
    "RACKED", "RACKET", "RADARS", "RAFTER", "RAMMED", "RAMPED", "RANGED", "RANKED",
    "RANTED", "RAREST", "RASHES", "RASTER", "RATHER", "RATTAN", "RATTEN", "RAVAGE",
    "RAVENS", "REACTS", "READER", "REAPER", "REARED", "REBATE", "REBUFF", "REBUKE",
    "RECANT", "RECEDE", "RECENT", "RECESS", "REDEEM", "REDUCE", "REEFED", "REFERS",
    "REFUGE", "REFUND", "REFUSE", "REGARD", "REGENT", "REGRET", "REJECT", "REMARK",
    "REMEDY", "RENDER", "RENEGE", "RENTED", "REPEAT", "REPENT", "REPUTE", "RESCUE",
    "RESEND", "RESENT", "RESTED", "RESUME", "RETARD", "RETURN", "REVERE", "REVERT",
    "REVVED", "RHYTHM", "RUBBED", "RUBBER", "RUDDER", "RUGGED", "RUSHED", "RUSTED",
];

/// Generate a human-friendly session password.
///
/// Returns a 6-character uppercase alphanumeric word randomly selected from a
/// curated wordlist. The words are chosen to be easy to read, spell, and
/// communicate over the phone.
///
/// # Examples
///
/// ```
/// use cv_shared::utils::generate_password;
///
/// let pwd = generate_password();
/// assert_eq!(pwd.0.len(), 6);
/// assert!(pwd.0.chars().all(|c| c.is_ascii_alphanumeric()));
/// ```
pub fn generate_password() -> Password {
    let mut rng = thread_rng();
    let word = PASSWORD_WORDLIST
        .choose(&mut rng)
        .expect("wordlist is non-empty");
    Password::new(*word)
}

/// Generate a random 6-character alphanumeric string.
///
/// Uses a cryptographically secure random number generator to produce a
/// mixed-case alphanumeric password. This is an alternative to the wordlist
/// approach when more entropy is desired.
///
/// # Examples
///
/// ```
/// use cv_shared::utils::generate_password_random;
///
/// let pwd = generate_password_random();
/// assert_eq!(pwd.0.len(), 6);
/// ```
pub fn generate_password_random() -> Password {
    use rand::Rng;
    const CHARSET: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZ23456789";
    // Excludes easily confused chars: 0, O, 1, I, L

    let mut rng = thread_rng();
    let chars: String = (0..6)
        .map(|_| {
            let idx = rng.gen_range(0..CHARSET.len());
            CHARSET[idx] as char
        })
        .collect();

    Password::new(chars)
}

// ---------------------------------------------------------------------------
// Timestamp utilities
// ---------------------------------------------------------------------------

/// Returns the current Unix timestamp in milliseconds.
///
/// # Examples
///
/// ```
/// use cv_shared::utils::now_millis;
///
/// let ts = now_millis();
/// assert!(ts > 1_700_000_000_000); // after 2023-11
/// ```
pub fn now_millis() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};

    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time before Unix epoch")
        .as_millis() as u64
}

/// Convert a chrono [`DateTime<chrono::Utc>`] to Unix milliseconds.
///
/// # Examples
///
/// ```
/// use cv_shared::utils::datetime_to_millis;
/// use chrono::Utc;
///
/// let dt = Utc::now();
/// let ts = datetime_to_millis(&dt);
/// assert!(ts > 1_700_000_000_000);
/// ```
pub fn datetime_to_millis(dt: &chrono::DateTime<chrono::Utc>) -> u64 {
    dt.timestamp_millis() as u64
}

/// Convert Unix milliseconds to a chrono [`DateTime<chrono::Utc>`].
///
/// # Examples
///
/// ```
/// use cv_shared::utils::millis_to_datetime;
///
/// let dt = millis_to_datetime(1_700_000_000_000);
/// assert_eq!(dt.timestamp_millis(), 1_700_000_000_000);
/// ```
pub fn millis_to_datetime(ts: u64) -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::from_timestamp_millis(ts as i64)
        .unwrap_or_else(|| chrono::DateTime::UNIX_EPOCH)
}

/// Format a Unix timestamp (in milliseconds) as an RFC 3339 string.
///
/// # Examples
///
/// ```
/// use cv_shared::utils::format_timestamp;
///
/// let s = format_timestamp(1_700_000_000_000);
/// assert!(s.starts_with("2023-11-14"));
/// ```
pub fn format_timestamp(ts: u64) -> String {
    let dt = millis_to_datetime(ts);
    dt.to_rfc3339()
}
