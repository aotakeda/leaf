use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiEntry {
    word: String,
    phonetic: Option<String>,
    #[serde(default)]
    phonetics: Vec<ApiPhonetic>,
    #[serde(default)]
    meanings: Vec<ApiMeaning>,
}

#[derive(Deserialize)]
struct ApiPhonetic {
    text: Option<String>,
    audio: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiMeaning {
    part_of_speech: String,
    #[serde(default)]
    definitions: Vec<ApiDefinition>,
    #[serde(default)]
    synonyms: Vec<String>,
}

#[derive(Deserialize)]
struct ApiDefinition {
    definition: String,
    example: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Definition {
    pub word: String,
    pub phonetic: Option<String>,
    pub audio: Option<String>,
    pub meanings: Vec<Meaning>,
    pub source: &'static str,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Meaning {
    pub part_of_speech: String,
    pub definitions: Vec<Sense>,
    pub synonyms: Vec<String>,
}

#[derive(Clone, Serialize)]
pub struct Sense {
    pub definition: String,
    pub example: Option<String>,
}

pub struct Dictionary {
    client: reqwest::Client,
    cache: Mutex<HashMap<String, Option<Definition>>>,
}

pub fn normalize(raw: &str) -> Option<String> {
    let cleaned: String = raw
        .replace('\u{AD}', "")
        .trim()
        .trim_matches(|c: char| !c.is_alphanumeric())
        .replace(['\u{2019}', '\u{2018}'], "'")
        .to_lowercase();
    let words = cleaned.split_whitespace().count();
    ((1..=3).contains(&words) && cleaned.chars().count() <= 60).then_some(cleaned)
}

fn base_lang(lang: &str) -> Option<String> {
    let base = lang.trim().split(['-', '_']).next()?.to_ascii_lowercase();
    (base.len() >= 2 && base.len() <= 3 && base.chars().all(|c| c.is_ascii_alphabetic()))
        .then_some(base)
}

fn pt_candidates(word: &str) -> Vec<String> {
    let mut out = vec![word.to_string()];
    let mut push = |s: String| {
        if !s.is_empty() && !out.contains(&s) {
            out.push(s);
        }
    };
    if let Some(stem) = word
        .strip_suffix("ões")
        .or_else(|| word.strip_suffix("ães"))
    {
        push(format!("{stem}ão"));
    } else if let Some(stem) = word.strip_suffix("is") {
        push(format!("{stem}l"));
        push(format!("{stem}i"));
    } else if let Some(stem) = word.strip_suffix("es") {
        push(stem.to_string());
        push(format!("{stem}e"));
    } else if let Some(stem) = word.strip_suffix('s') {
        push(stem.to_string());
    }
    out.truncate(3);
    out
}

impl Dictionary {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .user_agent("Leaf/0.1 (desktop ebook reader)")
                .timeout(std::time::Duration::from_secs(8))
                .build()
                .expect("http client"),
            cache: Mutex::new(HashMap::new()),
        }
    }

    pub(crate) async fn get(&self, url: reqwest::Url) -> Result<reqwest::Response, String> {
        let mut attempt = 0;
        loop {
            attempt += 1;
            let res = self.client.get(url.clone()).send().await.map_err(|_| {
                "Couldn't reach the dictionary. Check your internet connection.".to_string()
            })?;
            let retryable = res.status().is_server_error()
                || res.status() == reqwest::StatusCode::TOO_MANY_REQUESTS;
            if retryable && attempt < 3 {
                tokio::time::sleep(std::time::Duration::from_millis(600 * attempt)).await;
                continue;
            }
            return Ok(res);
        }
    }

    pub async fn define(&self, raw: &str, langs: &[String]) -> Result<Option<Definition>, String> {
        let word = normalize(raw).ok_or("Select a single word or short phrase")?;
        let mut order: Vec<String> = langs.iter().filter_map(|l| base_lang(l)).collect();
        order.push("en".into());
        order.dedup();
        let mut seen = Vec::new();
        order.retain(|l| {
            let fresh = !seen.contains(l);
            seen.push(l.clone());
            fresh
        });

        let mut last_err = None;
        for lang in order {
            match self.define_in(&word, &lang).await {
                Ok(Some(d)) => return Ok(Some(d)),
                Ok(None) => {}
                Err(e) => last_err = Some(e),
            }
        }
        match last_err {
            Some(e) => Err(e),
            None => Ok(None),
        }
    }

    async fn define_in(&self, word: &str, lang: &str) -> Result<Option<Definition>, String> {
        let key = format!("{lang}:{word}");
        if let Some(hit) = self.cache.lock().unwrap().get(&key) {
            return Ok(hit.clone());
        }
        let result = match lang {
            "en" => match self.english(word).await? {
                Some(d) => Some(d),
                None => crate::wiktionary::english_glosses(self, word, "en").await?,
            },
            "pt" => {
                let mut found = None;
                for candidate in pt_candidates(word) {
                    if let Some(mut d) = crate::wiktionary::portuguese(self, &candidate).await? {
                        d.word = candidate;
                        found = Some(d);
                        break;
                    }
                }
                match found {
                    Some(d) => Some(d),
                    None => crate::wiktionary::english_glosses(self, word, "pt").await?,
                }
            }
            other => crate::wiktionary::english_glosses(self, word, other).await?,
        };
        self.cache.lock().unwrap().insert(key, result.clone());
        Ok(result)
    }

    async fn english(&self, word: &str) -> Result<Option<Definition>, String> {
        let mut url = reqwest::Url::parse("https://api.dictionaryapi.dev/api/v2/entries/en/")
            .map_err(|e| e.to_string())?;
        url.path_segments_mut()
            .map_err(|_| "bad url")?
            .pop_if_empty()
            .push(word);
        let res = self.get(url).await?;
        if res.status() == reqwest::StatusCode::NOT_FOUND {
            Ok(None)
        } else if res.status().is_success() {
            let entries: Vec<ApiEntry> = res.json().await.map_err(|e| e.to_string())?;
            Ok(merge(entries))
        } else {
            Err(format!("Dictionary error ({})", res.status()))
        }
    }
}

fn merge(entries: Vec<ApiEntry>) -> Option<Definition> {
    let first = entries.first()?;
    let word = first.word.clone();
    let phonetic = entries
        .iter()
        .find_map(|e| e.phonetic.clone().filter(|p| !p.is_empty()))
        .or_else(|| {
            entries
                .iter()
                .flat_map(|e| &e.phonetics)
                .find_map(|p| p.text.clone().filter(|t| !t.is_empty()))
        });
    let audio = entries
        .iter()
        .flat_map(|e| &e.phonetics)
        .find_map(|p| p.audio.clone().filter(|a| !a.is_empty()));

    let mut meanings: Vec<Meaning> = Vec::new();
    for m in entries.into_iter().flat_map(|e| e.meanings) {
        let senses = m.definitions.into_iter().map(|d| Sense {
            definition: d.definition,
            example: d.example,
        });
        match meanings
            .iter_mut()
            .find(|x| x.part_of_speech == m.part_of_speech)
        {
            Some(existing) => {
                existing.definitions.extend(senses);
                existing.synonyms.extend(m.synonyms);
            }
            None => meanings.push(Meaning {
                part_of_speech: m.part_of_speech,
                definitions: senses.collect(),
                synonyms: m.synonyms,
            }),
        }
    }
    for m in &mut meanings {
        m.definitions.truncate(4);
        m.synonyms.dedup();
        m.synonyms.truncate(6);
    }

    Some(Definition {
        word,
        phonetic,
        audio,
        meanings,
        source: "Wiktionary via Free Dictionary API",
    })
}

#[cfg(test)]
mod tests {
    use super::normalize;

    #[test]
    fn normalizes_selection() {
        assert_eq!(normalize("  “Serendipity,” "), Some("serendipity".into()));
        assert_eq!(normalize("Don’t"), Some("don't".into()));
        assert_eq!(normalize("give up"), Some("give up".into()));
        assert_eq!(normalize("one two three four"), None);
        assert_eq!(normalize("..."), None);
        assert_eq!(
            normalize("milagrosa\u{AD}mente,"),
            Some("milagrosamente".into())
        );
        assert_eq!(normalize("Índios"), Some("índios".into()));
        assert_eq!(
            super::pt_candidates("desígnios"),
            vec!["desígnios", "desígnio"]
        );
        assert_eq!(
            super::pt_candidates("espoliadores"),
            vec!["espoliadores", "espoliador", "espoliadore"]
        );
        assert_eq!(super::pt_candidates("ações"), vec!["ações", "ação"]);
    }
}
