use crate::dictionary::{Definition, Dictionary, Meaning, Sense};
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize)]
struct ParseResponse {
    parse: Option<ParseBody>,
}

#[derive(Deserialize)]
struct ParseBody {
    wikitext: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RestEntry {
    part_of_speech: String,
    #[serde(default)]
    definitions: Vec<RestDefinition>,
}

#[derive(Deserialize)]
struct RestDefinition {
    definition: String,
    #[serde(default)]
    examples: Vec<String>,
}

pub async fn portuguese(dict: &Dictionary, word: &str) -> Result<Option<Definition>, String> {
    let mut url =
        reqwest::Url::parse("https://pt.wiktionary.org/w/api.php").map_err(|e| e.to_string())?;
    url.query_pairs_mut()
        .append_pair("action", "parse")
        .append_pair("page", word)
        .append_pair("prop", "wikitext")
        .append_pair("format", "json")
        .append_pair("formatversion", "2")
        .append_pair("redirects", "1");
    let res = dict.get(url).await?;
    if !res.status().is_success() {
        return Err(format!("Dictionary error ({})", res.status()));
    }
    let body: ParseResponse = res.json().await.map_err(|e| e.to_string())?;
    Ok(body
        .parse
        .and_then(|p| parse_pt_wikitext(word, &p.wikitext)))
}

pub async fn english_glosses(
    dict: &Dictionary,
    word: &str,
    lang: &str,
) -> Result<Option<Definition>, String> {
    let mut url = reqwest::Url::parse("https://en.wiktionary.org/api/rest_v1/page/definition/")
        .map_err(|e| e.to_string())?;
    url.path_segments_mut()
        .map_err(|_| "bad url")?
        .pop_if_empty()
        .push(word);
    let res = dict.get(url).await?;
    if res.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }
    if !res.status().is_success() {
        return Err(format!("Dictionary error ({})", res.status()));
    }
    let mut by_lang: HashMap<String, Vec<RestEntry>> =
        res.json().await.map_err(|e| e.to_string())?;
    let Some(entries) = by_lang.remove(lang) else {
        return Ok(None);
    };
    let meanings: Vec<Meaning> = entries
        .into_iter()
        .filter_map(|e| {
            let definitions: Vec<Sense> = e
                .definitions
                .into_iter()
                .filter_map(|d| {
                    let definition = strip_html(&d.definition);
                    (!definition.is_empty()).then(|| Sense {
                        definition,
                        example: d
                            .examples
                            .first()
                            .map(|x| strip_html(x))
                            .filter(|x| !x.is_empty()),
                    })
                })
                .take(4)
                .collect();
            (!definitions.is_empty()).then(|| Meaning {
                part_of_speech: e.part_of_speech.to_lowercase(),
                definitions,
                synonyms: Vec::new(),
            })
        })
        .collect();
    Ok((!meanings.is_empty()).then(|| Definition {
        word: word.to_string(),
        phonetic: None,
        audio: None,
        meanings,
        source: "Wiktionary",
    }))
}

fn strip_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    for c in s.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => out.push(c),
            _ => {}
        }
    }
    decode_entities(&out)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn decode_entities(s: &str) -> String {
    s.replace("&nbsp;", " ")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&amp;", "&")
}

fn render_template(inner: &str) -> String {
    let mut parts = inner.split('|');
    let name = parts.next().unwrap_or("").trim().to_lowercase();
    let args: Vec<&str> = parts.filter(|a| !a.contains('=')).collect();
    match name.as_str() {
        "escopo" | "escopo2" | "escopo3" => args
            .iter()
            .rev()
            .find(|a| a.trim().len() > 3)
            .map(|a| format!("({})", a.trim()))
            .unwrap_or_default(),
        "termo" | "mention" | "l" | "link" => args
            .last()
            .map(|a| a.trim().to_string())
            .unwrap_or_default(),
        _ => String::new(),
    }
}

fn template_name(inner: &str) -> String {
    inner.split('|').next().unwrap_or("").trim().to_string()
}

fn strip_templates(s: &str, render: fn(&str) -> String) -> String {
    let mut out = String::new();
    let mut depth = 0usize;
    let mut buf = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '{' && chars.peek() == Some(&'{') {
            chars.next();
            depth += 1;
            if depth > 1 {
                buf.push_str("{{");
            }
        } else if c == '}' && chars.peek() == Some(&'}') && depth > 0 {
            chars.next();
            depth -= 1;
            if depth == 0 {
                out.push_str(&render(&buf));
                buf.clear();
            } else {
                buf.push_str("}}");
            }
        } else if depth > 0 {
            buf.push(c);
        } else {
            out.push(c);
        }
    }
    out
}

fn strip_links(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;
    while let Some(start) = rest.find("[[") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        match after.find("]]") {
            Some(end) => {
                let inner = &after[..end];
                if !inner.contains(':') || inner.starts_with(':') {
                    out.push_str(inner.rsplit('|').next().unwrap_or(inner));
                }
                rest = &after[end + 2..];
            }
            None => {
                out.push_str(&rest[start..]);
                rest = "";
            }
        }
    }
    out.push_str(rest);
    out
}

fn clean(s: &str) -> String {
    let mut text = s.to_string();
    while let Some(start) = text.find("<ref") {
        let end = text[start..]
            .find("</ref>")
            .map(|e| start + e + 6)
            .or_else(|| text[start..].find("/>").map(|e| start + e + 2))
            .unwrap_or(text.len());
        text.replace_range(start..end, "");
    }
    let text = strip_templates(&text, render_template);
    let text = strip_links(&text);
    let text = strip_html(&text.replace("'''", "").replace("''", ""));
    text.trim_matches(|c: char| c == ';' || c == ',' || c.is_whitespace())
        .to_string()
}

fn heading(line: &str) -> Option<(usize, String)> {
    let t = line.trim();
    if !t.starts_with('=') || !t.ends_with('=') {
        return None;
    }
    let level = t.chars().take_while(|&c| c == '=').count();
    let inner = t.trim_matches('=').trim();
    let name = if inner.starts_with("{{") && inner.ends_with("}}") {
        template_name(&inner[2..inner.len() - 2])
    } else {
        clean(inner)
    };
    Some((level, name))
}

pub fn parse_pt_wikitext(word: &str, wikitext: &str) -> Option<Definition> {
    let mut in_pt = false;
    let mut pos = String::new();
    let mut section = String::new();
    let mut meanings: Vec<Meaning> = Vec::new();
    let mut synonyms: Vec<String> = Vec::new();

    for line in wikitext.lines() {
        if let Some((level, name)) = heading(line) {
            if level == 1 {
                if in_pt {
                    break;
                }
                in_pt = name == "-pt-";
            } else if in_pt && level == 2 {
                pos = name.clone();
                section = name;
            } else if in_pt {
                section = name;
            }
            continue;
        }
        if !in_pt {
            continue;
        }

        let t = line.trim_start();
        if section.to_lowercase().starts_with("sinônimo") {
            if let Some(rest) = t.strip_prefix('*') {
                let s = clean(rest);
                for part in s.split([',', ';']) {
                    let p = part.trim();
                    if !p.is_empty()
                        && p.len() < 40
                        && synonyms.len() < 6
                        && !synonyms.iter().any(|x| x == p)
                    {
                        synonyms.push(p.to_string());
                    }
                }
            }
            continue;
        }
        if section != pos {
            continue;
        }

        if let Some(rest) = t.strip_prefix('#') {
            if let Some(example) = rest.strip_prefix('*') {
                if example.starts_with(':') {
                    continue;
                }
                let ex = clean(example);
                if let Some(sense) = meanings.last_mut().and_then(|m| m.definitions.last_mut()) {
                    if sense.example.is_none() && !ex.is_empty() {
                        sense.example = Some(ex);
                    }
                }
                continue;
            }
            if rest.starts_with(':') || rest.starts_with('#') {
                continue;
            }
            let def = clean(rest);
            if def.is_empty() {
                continue;
            }
            let label = pos.to_lowercase();
            match meanings.iter_mut().find(|m| m.part_of_speech == label) {
                Some(m) => m.definitions.push(Sense {
                    definition: def,
                    example: None,
                }),
                None => meanings.push(Meaning {
                    part_of_speech: label,
                    definitions: vec![Sense {
                        definition: def,
                        example: None,
                    }],
                    synonyms: Vec::new(),
                }),
            }
        }
    }

    for m in &mut meanings {
        m.definitions.truncate(4);
    }
    if let Some(first) = meanings.first_mut() {
        first.synonyms = synonyms;
    }
    (!meanings.is_empty()).then(|| Definition {
        word: word.to_string(),
        phonetic: None,
        audio: None,
        meanings,
        source: "Wikcionário",
    })
}

#[cfg(test)]
mod tests {
    use super::parse_pt_wikitext;

    #[test]
    fn parses_adverb() {
        let w = "={{-pt-}}=\n==Advérbio==\n{{paroxítona|mi|la}}\n# de [[modo]] [[milagroso]]; [[miraculosamente]]\n\n=={{etimologia|pt}}==\n: ''[[milagroso|milagrosa]]'' + ''[[-mente]]''.\n";
        let d = parse_pt_wikitext("milagrosamente", w).unwrap();
        assert_eq!(d.meanings.len(), 1);
        assert_eq!(d.meanings[0].part_of_speech, "advérbio");
        assert_eq!(
            d.meanings[0].definitions[0].definition,
            "de modo milagroso; miraculosamente"
        );
    }

    #[test]
    fn parses_examples_scopes_and_verb_forms() {
        let w = "={{-pt-}}=\n==Substantivo==\n# [[construção]] que [[servir|serve]] de [[moradia]]\n#* ''Sua '''casa''' é muito bonita!''\n#: '''2004''', Autor\n# {{escopo2|Brasileirismo}} [[motel]]\n=={{Forma verbal|pt}}==\n# [[terceira pessoa]] do verbo '''[[casar]]'''\n={{-es-}}=\n==Sustantivo==\n# house\n";
        let d = parse_pt_wikitext("casa", w).unwrap();
        assert_eq!(d.meanings.len(), 2);
        let s = &d.meanings[0].definitions;
        assert_eq!(s[0].definition, "construção que serve de moradia");
        assert_eq!(s[0].example.as_deref(), Some("Sua casa é muito bonita!"));
        assert_eq!(s[1].definition, "(Brasileirismo) motel");
        assert_eq!(d.meanings[1].part_of_speech, "forma verbal");
        assert_eq!(
            d.meanings[1].definitions[0].definition,
            "terceira pessoa do verbo casar"
        );
    }
}
