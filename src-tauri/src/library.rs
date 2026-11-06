use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Format {
    Epub,
    Pdf,
}

impl Format {
    fn from_path(path: &Path) -> Option<Self> {
        match path.extension()?.to_str()?.to_ascii_lowercase().as_str() {
            "epub" => Some(Format::Epub),
            "pdf" => Some(Format::Pdf),
            _ => None,
        }
    }

    fn ext(self) -> &'static str {
        match self {
            Format::Epub => "epub",
            Format::Pdf => "pdf",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Book {
    pub id: String,
    pub title: String,
    pub author: Option<String>,
    pub format: Format,
    pub added_at: u64,
    pub last_opened_at: Option<u64>,
    pub location: Option<String>,
    pub progress: f32,
    pub has_cover: bool,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Library {
    books: Vec<Book>,
    #[serde(skip)]
    root: PathBuf,
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn hash_file(path: &Path) -> Result<String, String> {
    let mut file = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut hasher = blake3::Hasher::new();
    let mut buf = vec![0u8; 1 << 16];
    loop {
        let n = file.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher.finalize().to_hex()[..16].to_string())
}

fn title_from_filename(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.replace(['_', '-'], " ").trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Untitled".to_string())
}

impl Library {
    pub fn load(root: PathBuf) -> Self {
        for dir in ["books", "covers", "locations"] {
            let _ = fs::create_dir_all(root.join(dir));
        }
        let mut lib: Library = fs::read(root.join("library.json"))
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        lib.root = root;
        lib
    }

    fn save(&self) -> Result<(), String> {
        let tmp = self.root.join("library.json.tmp");
        let data = serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?;
        fs::write(&tmp, data).map_err(|e| e.to_string())?;
        fs::rename(&tmp, self.root.join("library.json")).map_err(|e| e.to_string())
    }

    pub fn list(&self) -> Vec<Book> {
        let mut books = self.books.clone();
        books.sort_by(|a, b| {
            b.last_opened_at
                .unwrap_or(b.added_at)
                .cmp(&a.last_opened_at.unwrap_or(a.added_at))
        });
        books
    }

    fn get_mut(&mut self, id: &str) -> Result<&mut Book, String> {
        self.books
            .iter_mut()
            .find(|b| b.id == id)
            .ok_or_else(|| "Book not found".to_string())
    }

    fn book_path(&self, book: &Book) -> PathBuf {
        self.root
            .join("books")
            .join(format!("{}.{}", book.id, book.format.ext()))
    }

    pub fn cover_path(&self, id: &str) -> PathBuf {
        self.root.join("covers").join(id)
    }

    pub fn locations_path(&self, id: &str) -> PathBuf {
        self.root.join("locations").join(format!("{id}.json"))
    }

    pub fn import(&mut self, src: &Path) -> Result<Book, String> {
        let format = Format::from_path(src).ok_or("Only EPUB and PDF files are supported")?;
        let id = hash_file(src)?;
        if let Some(existing) = self.books.iter().find(|b| b.id == id) {
            return Ok(existing.clone());
        }

        let mut book = Book {
            id: id.clone(),
            title: title_from_filename(src),
            author: None,
            format,
            added_at: now(),
            last_opened_at: None,
            location: None,
            progress: 0.0,
            has_cover: false,
        };

        fs::copy(src, self.book_path(&book)).map_err(|e| e.to_string())?;

        if format == Format::Epub {
            if let Some(meta) = crate::epub_meta::read(&self.book_path(&book)) {
                if let Some(t) = meta.title {
                    book.title = t;
                }
                book.author = meta.author;
                if let Some(bytes) = meta.cover {
                    book.has_cover = fs::write(self.cover_path(&id), bytes).is_ok();
                }
            }
        }

        self.books.push(book.clone());
        self.save()?;
        Ok(book)
    }

    pub fn open(&mut self, id: &str) -> Result<Vec<u8>, String> {
        let book = self.get_mut(id)?;
        book.last_opened_at = Some(now());
        let book = book.clone();
        self.save()?;
        fs::read(self.book_path(&book)).map_err(|e| e.to_string())
    }

    pub fn set_progress(
        &mut self,
        id: &str,
        location: String,
        progress: f32,
    ) -> Result<(), String> {
        let book = self.get_mut(id)?;
        book.location = Some(location);
        book.progress = progress.clamp(0.0, 1.0);
        self.save()
    }

    pub fn set_meta(
        &mut self,
        id: &str,
        title: Option<String>,
        author: Option<String>,
    ) -> Result<(), String> {
        let book = self.get_mut(id)?;
        if let Some(t) = title
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
        {
            book.title = t;
        }
        if let Some(a) = author
            .map(|a| a.trim().to_string())
            .filter(|a| !a.is_empty())
        {
            book.author = Some(a);
        }
        self.save()
    }

    pub fn set_cover(&mut self, id: &str, bytes: &[u8]) -> Result<(), String> {
        fs::write(self.cover_path(id), bytes).map_err(|e| e.to_string())?;
        self.get_mut(id)?.has_cover = true;
        self.save()
    }

    pub fn remove(&mut self, id: &str) -> Result<(), String> {
        let idx = self
            .books
            .iter()
            .position(|b| b.id == id)
            .ok_or("Book not found")?;
        let book = self.books.remove(idx);
        let _ = fs::remove_file(self.book_path(&book));
        let _ = fs::remove_file(self.cover_path(id));
        let _ = fs::remove_file(self.locations_path(id));
        self.save()
    }
}
