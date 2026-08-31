const BYTE_SUFFIXES: [&str; 9] = ["", "Ki", "Mi", "Gi", "Ti", "Pi", "Ei", "Zi", "Yi"];
static PARSE_BYTES_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?P<num>\d+(?:\.\d+)?)\s*(?P<unit>[KMGTPEZY]?)$").unwrap());
static OUTPUT_TEMPLATE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"%\((?P<key>[^)]+)\)(?P<format>[#0\-+ ]?\d*(?:\.\d+)?[sdif])").unwrap()
});
static DURATION_CLOCK_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^(?:(?:(?P<days>\d+):)?(?P<hours>\d+):)?(?P<mins>\d+):(?P<secs>\d{1,2})(?P<ms>[.:]\d+)?Z?$",
    )
    .unwrap()
});
static DURATION_SECONDS_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^(?P<secs>\d+)(?P<ms>[.:]\d+)?Z?$").unwrap());
static DURATION_UNITS_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?ix)^(?:P?
        (?:\d+\s*y(?:ears?)?,?\s*)?
        (?:\d+\s*m(?:onths?)?,?\s*)?
        (?:\d+\s*w(?:eeks?)?,?\s*)?
        (?:(?P<days>\d+)\s*d(?:ays?)?,?\s*)?
        T)?
        (?:(?P<hours>\d+)\s*h(?:(?:ou)?rs?)?,?\s*)?
        (?:(?P<mins>\d+)\s*m(?:in(?:ute)?s?)?,?\s*)?
        (?:(?P<secs>\d+)(?P<ms>\.\d+)?\s*s(?:ec(?:ond)?s?)?\s*)?
        Z?$",
    )
    .unwrap()
});
static DURATION_TEXT_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?ix)^(?:(?P<hours>[0-9.]+)\s*(?:hours?)|(?P<mins>[0-9.]+)\s*(?:mins?\.?|minutes?)\s*)Z?$",
    )
    .unwrap()
});
static ISO8601_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"^(?P<year>\d{4})-(?P<month>\d{2})-(?P<day>\d{2})T(?P<hour>\d{2}):(?P<minute>\d{2}):(?P<second>\d{2})(?:\.\d+)?(?P<timezone>Z|(?P<sign>[+-])(?P<tzhour>\d{2}):?(?P<tzminute>\d{2}))?$",
    )
    .unwrap()
});
static URL_SCHEME_RE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[A-Za-z][A-Za-z0-9+.-]*:").unwrap());
