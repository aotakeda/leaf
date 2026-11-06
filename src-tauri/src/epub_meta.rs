use std::fs::File;
use std::io::Read;
use std::path::Path;

pub struct EpubMeta {
    pub title: Option<String>,
    pub author: Option<String>,
    pub cover: Option<Vec<u8>>,
}

fn read_entry(archive: &mut zip::ZipArchive<File>, name: &str) -> Option<Vec<u8>> {
    let mut entry = archive.by_name(name).ok()?;
    let mut buf = Vec::with_capacity(entry.size() as usize);
    entry.read_to_end(&mut buf).ok()?;
    Some(buf)
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() && s.is_char_boundary(i + 3) {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn resolve(base_dir: &str, href: &str) -> String {
    let href = percent_decode(href.split('#').next().unwrap_or(href));
    let mut parts: Vec<&str> = base_dir.split('/').filter(|p| !p.is_empty()).collect();
    for seg in href.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            s => parts.push(s),
        }
    }
    parts.join("/")
}

fn clean(s: &str) -> Option<String> {
    let s = s.split_whitespace().collect::<Vec<_>>().join(" ");
    (!s.is_empty()).then_some(s)
}

pub fn read(path: &Path) -> Option<EpubMeta> {
    let mut archive = zip::ZipArchive::new(File::open(path).ok()?).ok()?;

    let container = String::from_utf8(read_entry(&mut archive, "META-INF/container.xml")?).ok()?;
    let container = roxmltree::Document::parse(&container).ok()?;
    let opf_path = container
        .descendants()
        .find(|n| n.has_tag_name("rootfile"))?
        .attribute("full-path")?
        .to_string();
    let opf_dir = opf_path.rsplit_once('/').map(|(d, _)| d).unwrap_or("");

    let opf = String::from_utf8(read_entry(&mut archive, &opf_path)?).ok()?;
    let opf = roxmltree::Document::parse(&opf).ok()?;

    let text_of = |tag: &str| {
        opf.descendants()
            .find(|n| n.tag_name().name() == tag && n.text().is_some())
            .and_then(|n| n.text())
            .and_then(clean)
    };
    let title = text_of("title");
    let author = text_of("creator");

    let items: Vec<_> = opf
        .descendants()
        .filter(|n| n.tag_name().name() == "item")
        .collect();
    let cover_id = opf
        .descendants()
        .find(|n| n.tag_name().name() == "meta" && n.attribute("name") == Some("cover"))
        .and_then(|n| n.attribute("content"));
    let cover_href = items
        .iter()
        .find(|n| {
            n.attribute("properties")
                .is_some_and(|p| p.split_whitespace().any(|x| x == "cover-image"))
        })
        .or_else(|| {
            items
                .iter()
                .find(|n| cover_id.is_some() && n.attribute("id") == cover_id)
        })
        .or_else(|| {
            items.iter().find(|n| {
                n.attribute("media-type")
                    .is_some_and(|m| m.starts_with("image/"))
                    && n.attribute("id")
                        .is_some_and(|id| id.to_ascii_lowercase().contains("cover"))
            })
        })
        .and_then(|n| n.attribute("href"));
    let cover = cover_href.and_then(|href| read_entry(&mut archive, &resolve(opf_dir, href)));

    Some(EpubMeta {
        title,
        author,
        cover,
    })
}

#[cfg(test)]
mod tests {
    use super::{percent_decode, resolve};

    #[test]
    fn resolves_relative_paths() {
        assert_eq!(
            resolve("OEBPS", "images/cover.jpg"),
            "OEBPS/images/cover.jpg"
        );
        assert_eq!(
            resolve("OEBPS/text", "../img/a%20b.png"),
            "OEBPS/img/a b.png"
        );
        assert_eq!(resolve("", "cover.png"), "cover.png");
        assert_eq!(percent_decode("100%"), "100%");
    }
}
