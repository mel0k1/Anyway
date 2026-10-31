#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rule {
    Status,
    Msg(&'static [&'static str]),
    Redirect(&'static str),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SiteMethod {
    Get,
    Post,
}

#[derive(Debug, Clone)]
pub struct Site {
    pub name: &'static str,
    pub url: &'static str,
    pub link: &'static str,
    pub probe: &'static str,
    pub method: SiteMethod,
    pub payload: &'static str,
    pub rule: Rule,
    pub regex: &'static str,
    pub category: &'static str,
    pub unknown_codes: &'static [u16],
}

impl Site {
    pub const fn new(name: &'static str, url: &'static str, category: &'static str, rule: Rule) -> Self {
        Site {
            name,
            url,
            link: "",
            probe: "",
            method: SiteMethod::Get,
            payload: "",
            rule,
            regex: "",
            category,
            unknown_codes: &[],
        }
    }

    pub const fn link(mut self, link: &'static str) -> Self {
        self.link = link;
        self
    }

    pub const fn probe(mut self, probe: &'static str) -> Self {
        self.probe = probe;
        self
    }

    pub const fn post(mut self, payload: &'static str) -> Self {
        self.method = SiteMethod::Post;
        self.payload = payload;
        self
    }

    pub const fn rx(mut self, regex: &'static str) -> Self {
        self.regex = regex;
        self
    }

    pub const fn unknown_on(mut self, codes: &'static [u16]) -> Self {
        self.unknown_codes = codes;
        self
    }
}

pub const SITES: &[Site] = &[
    Site::new("GitHub", "https://github.com/{}", "dev", Rule::Status),
    Site::new("GitLab", "https://gitlab.com/{}", "dev", Rule::Status),
    Site::new("Bitbucket", "https://bitbucket.org/{}/", "dev", Rule::Status),
    Site::new("Docker Hub", "https://hub.docker.com/u/{}", "dev", Rule::Status),
    Site::new("npm", "https://www.npmjs.com/~{}", "dev", Rule::Status),
    Site::new("PyPI", "https://pypi.org/user/{}/", "dev", Rule::Status),
    Site::new("Dev Community", "https://dev.to/{}", "dev", Rule::Status),
    Site::new("Keybase", "https://keybase.io/{}", "dev", Rule::Status),
    Site::new("Codewars", "https://www.codewars.com/users/{}", "dev", Rule::Status),
    Site::new("Chess.com", "", "gaming", Rule::Status)
        .probe("https://api.chess.com/pub/player/{}")
        .link("https://www.chess.com/member/{}"),
    Site::new("Steam", "https://steamcommunity.com/id/{}/", "gaming", Rule::Msg(&["The specified profile could not be found."])),
    Site::new("Twitch", "https://www.twitch.tv/{}", "gaming", Rule::Msg(&["Sorry. Unless you've got a time machine"])),
    Site::new("Telegram", "https://t.me/{}", "messenger", Rule::Msg(&["Telegram Messenger"])),
    Site::new("Discord", "https://discord.com", "messenger", Rule::Msg(&["{\"taken\":false}"]))
        .probe("https://discord.com/api/v9/unique-username/username-attempt-unauthed")
        .post("{\"username\":\"{}\"}"),
    Site::new("Instagram", "https://instagram.com/{}", "social", Rule::Status),
    Site::new("X", "https://x.com/{}", "social", Rule::Status),
    Site::new("Pinterest", "https://www.pinterest.com/{}/", "social", Rule::Status),
    Site::new("Behance", "https://www.behance.net/{}", "social", Rule::Status),
    Site::new("Dribbble", "https://dribbble.com/{}", "social", Rule::Status),
    Site::new("about.me", "https://about.me/{}", "social", Rule::Status),
    Site::new("VK", "https://vk.com/{}", "social", Rule::Msg(&["Страница не найдена", "Page not found"])),
    Site::new("OK.ru", "https://ok.ru/{}", "social", Rule::Status),
    Site::new("Reddit", "", "forum", Rule::Status)
        .probe("https://www.reddit.com/user/{}/about.json")
        .link("https://www.reddit.com/user/{}")
        .unknown_on(&[403, 429]),
    Site::new("Hacker News", "", "forum", Rule::Msg(&["No such user."]))
        .probe("https://news.ycombinator.com/user?id={}")
        .link("https://news.ycombinator.com/user?id={}"),
    Site::new("Medium", "https://medium.com/@{}", "blog", Rule::Status),
    Site::new("Blogspot", "https://{}.blogspot.com", "blog", Rule::Status),
    Site::new("SoundCloud", "https://soundcloud.com/{}", "media", Rule::Status),
    Site::new("Last.fm", "https://www.last.fm/user/{}", "media", Rule::Status),
    Site::new("Flickr", "https://www.flickr.com/people/{}", "media", Rule::Status),
    Site::new("Vimeo", "https://vimeo.com/{}", "media", Rule::Status),
    Site::new("Dailymotion", "https://www.dailymotion.com/{}", "media", Rule::Status),
    Site::new("MyAnimeList", "https://myanimelist.net/profile/{}", "media", Rule::Status),
    Site::new("Trakt", "https://trakt.tv/users/{}", "media", Rule::Status),
    Site::new("Letterboxd", "https://letterboxd.com/{}/", "media", Rule::Status),
    Site::new("Wattpad", "https://www.wattpad.com/user/{}", "media", Rule::Status),
    Site::new("Gravatar", "https://gravatar.com/{}", "other", Rule::Status),
];

pub const WAF_FINGERPRINTS: &[&str] = &[
    "Just a moment",
    "Attention Required",
    "_Incapsula_Resource",
    "perimeterx",
    "cf-chl",
    "cf-browser-verification",
];
