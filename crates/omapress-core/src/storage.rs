use crate::model::*;
use anyhow::{Context, Result, bail, ensure};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::os::unix::fs::PermissionsExt;
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Component, Path, PathBuf},
};

pub const MAX_SOURCE: usize = 256 * 1024 * 1024;
#[derive(Clone, Debug)]
pub struct Snapshot {
    pub root: PathBuf,
    pub files: BTreeMap<String, Vec<u8>>,
    pub hash: String,
    pub config: Publication,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct State {
    pub schema: u32,
    #[serde(default)]
    pub identities: BTreeMap<String, Identity>,
    #[serde(default)]
    pub deployments: Vec<Deployment>,
    #[serde(default)]
    pub pending: Option<Deployment>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Identity {
    pub url: String,
    pub published_at: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Deployment {
    pub id: String,
    pub repository: String,
    pub branch: String,
    pub base_url: String,
    pub source_hash: String,
    pub source_commit: Option<String>,
    pub source_tree: Option<String>,
    pub output_hash: String,
    pub remote_commit: String,
    pub previous_head: String,
    pub articles: BTreeMap<String, String>,
    pub identities: BTreeMap<String, Identity>,
    pub feeds: BTreeMap<String, String>,
    pub created_at: String,
    pub verified_at: Option<String>,
}
pub fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
pub fn tree_hash(files: &BTreeMap<String, Vec<u8>>) -> String {
    let mut h = Sha256::new();
    for (path, bytes) in files {
        h.update((path.len() as u64).to_be_bytes());
        h.update(path.as_bytes());
        h.update((bytes.len() as u64).to_be_bytes());
        h.update(bytes);
    }
    format!("{:x}", h.finalize())
}
pub fn safe_relative(s: &str) -> Result<()> {
    ensure!(
        !s.is_empty() && !s.contains('\\') && !s.contains('\0') && !s.chars().any(char::is_control),
        "Unsafe path"
    );
    ensure!(
        Path::new(s)
            .components()
            .all(|c| matches!(c, Component::Normal(_))),
        "Path must be relative and cannot contain . or .."
    );
    Ok(())
}
pub fn safe_path(root: &Path, relative: &str) -> Result<PathBuf> {
    safe_relative(relative)?;
    let mut p = root.to_path_buf();
    for c in Path::new(relative).components() {
        p.push(c);
        if let Ok(m) = fs::symlink_metadata(&p) {
            ensure!(
                !m.file_type().is_symlink(),
                "Symlinks are not allowed: {}",
                p.display()
            );
        }
    }
    Ok(p)
}
pub fn read_bounded(path: &Path, limit: usize) -> Result<Vec<u8>> {
    use std::io::Read;
    let f = File::open(path)?;
    ensure!(
        f.metadata()?.is_file(),
        "Expected regular file: {}",
        path.display()
    );
    let mut b = Vec::new();
    f.take(limit as u64 + 1).read_to_end(&mut b)?;
    ensure!(
        b.len() <= limit,
        "File exceeds size limit: {}",
        path.display()
    );
    Ok(b)
}
fn collect(
    root: &Path,
    path: &Path,
    out: &mut BTreeMap<String, Vec<u8>>,
    size: &mut usize,
) -> Result<()> {
    let m = fs::symlink_metadata(path)?;
    ensure!(
        !m.file_type().is_symlink(),
        "Symlink rejected: {}",
        path.display()
    );
    if m.is_dir() {
        for e in fs::read_dir(path)? {
            collect(root, &e?.path(), out, size)?;
        }
    } else {
        ensure!(m.is_file(), "Special file rejected: {}", path.display());
        ensure!(out.len() < 30000, "Publication has too many files");
        let name = path
            .strip_prefix(root)?
            .to_str()
            .context("Paths must be UTF-8")?
            .to_string();
        safe_relative(&name)?;
        let data = read_bounded(path, 32 * 1024 * 1024)?;
        *size += data.len();
        ensure!(
            *size <= MAX_SOURCE,
            "Publication exceeds 256 MiB input budget"
        );
        out.insert(name, data);
    }
    Ok(())
}
pub fn snapshot(path: &Path) -> Result<Snapshot> {
    let root = path
        .canonicalize()
        .context("Publication folder does not exist")?;
    let mut files = BTreeMap::new();
    let mut size = 0;
    for name in ["publication.toml", "content", "media", "themes", "static"] {
        let path = root.join(name);
        if path.exists() || path.is_symlink() {
            collect(&root, &path, &mut files, &mut size)?;
        }
    }
    let data = files
        .get("publication.toml")
        .context("Missing publication.toml")?;
    let config: Publication = toml::from_str(std::str::from_utf8(data)?)?;
    validate_config(&config)?;
    let hash = tree_hash(&files);
    Ok(Snapshot {
        root,
        files,
        hash,
        config,
    })
}
impl Snapshot {
    pub fn articles(&self) -> (Vec<Article>, Vec<Diagnostic>) {
        let mut articles = Vec::new();
        let mut issues = Vec::new();
        for (path, bytes) in &self.files {
            if path.starts_with("content/") && path.ends_with(".md") {
                match parse_article(path, bytes) {
                    Ok(a) => articles.push(a),
                    Err(e) => issues.push(Diagnostic::error(path, format!("{e:#}"))),
                }
            }
        }
        articles.sort_by(|a, b| {
            b.meta
                .published_at
                .cmp(&a.meta.published_at)
                .then(a.path.cmp(&b.path))
        });
        (articles, issues)
    }
    pub fn expected(&self, h: &str) -> Result<()> {
        ensure!(
            h == self.hash,
            "Source changed since review; inspect and review again"
        );
        Ok(())
    }
}
pub struct Lock(File);
impl Drop for Lock {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.0);
    }
}
pub fn lock(root: &Path) -> Result<Lock> {
    let state_dir = safe_path(root, ".omapress")?;
    fs::create_dir_all(&state_dir)?;
    fs::set_permissions(&state_dir, fs::Permissions::from_mode(0o700))?;
    let p = safe_path(root, ".omapress/lock")?;
    let f = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(p)?;
    f.try_lock_exclusive()
        .context("Another OmaPress operation is active")?;
    Ok(Lock(f))
}
pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path.parent().context("Missing parent")?;
    fs::create_dir_all(parent)?;
    let mut t = tempfile::NamedTempFile::new_in(parent)?;
    t.as_file()
        .set_permissions(fs::Permissions::from_mode(0o600))?;
    t.write_all(bytes)?;
    t.as_file().sync_all()?;
    t.persist(path).map_err(|e| e.error)?;
    File::open(parent)?.sync_all()?;
    Ok(())
}
pub fn state(root: &Path) -> Result<State> {
    let path = safe_path(root, ".omapress/state.json")?;
    if !path.exists() {
        return Ok(State {
            schema: SCHEMA,
            ..Default::default()
        });
    }
    let state: State = serde_json::from_slice(&read_bounded(&path, 16 * 1024 * 1024)?)
        .context("Deployment state is corrupt; restore it before publishing")?;
    ensure!(
        state.schema == SCHEMA,
        "Unsupported deployment-state schema"
    );
    Ok(state)
}
pub fn save_state(root: &Path, state: &State) -> Result<()> {
    atomic_write(
        &safe_path(root, ".omapress/state.json")?,
        &serde_json::to_vec_pretty(state)?,
    )
}
pub fn init(path: &Path, name: &str, base_url: &str, author: &str) -> Result<Snapshot> {
    ensure!(
        !path.exists() || (path.is_dir() && fs::read_dir(path)?.next().is_none()),
        "Initialisation requires a new or empty folder"
    );
    let mut series = BTreeMap::new();
    series.insert(
        "today-in-omarchy".into(),
        Series {
            title: "Today in Omarchy".into(),
            route: "today-in-omarchy".into(),
        },
    );
    series.insert(
        "unofficial-week-in-omarchy".into(),
        Series {
            title: "The Unofficial Week in Omarchy".into(),
            route: "the-unofficial-week-in-omarchy".into(),
        },
    );
    let config = Publication {
        schema: SCHEMA,
        name: name.into(),
        tagline: String::new(),
        description: "An independent publication".into(),
        base_url: base_url.into(),
        language: "en-GB".into(),
        timezone: "Europe/London".into(),
        author: author.into(),
        theme: "default".into(),
        about: "An independent publication. Written and published with OmaPress.".into(),
        feeds: FeedConfig::default(),
        deploy: DeployConfig::default(),
        series,
        x_export_order: vec![
            "masthead".into(),
            "edition".into(),
            "title".into(),
            "date".into(),
        ],
    };
    validate_config(&config)?;
    fs::create_dir_all(path)?;
    for d in ["content", "media", "themes", "static"] {
        fs::create_dir(path.join(d))?;
    }
    atomic_write(
        &path.join("publication.toml"),
        toml::to_string_pretty(&config)?.as_bytes(),
    )?;
    atomic_write(&path.join(".gitignore"), b"/output/\n/.omapress/\n")?;
    snapshot(path)
}
pub fn save_article(root: &Path, path: &str, text: &str, expected: &str) -> Result<Snapshot> {
    let _lock = lock(root)?;
    let s = snapshot(root)?;
    s.expected(expected)?;
    ensure!(
        path.starts_with("content/") && path.ends_with(".md"),
        "Article must be a Markdown file under content/"
    );
    let mut article = parse_article(path, text.as_bytes())?;
    if let Some(old) = s.files.get(path) {
        let old = parse_article(path, old)?;
        ensure!(old.meta.id == article.meta.id, "Article ID is immutable");
        if old.hash != article.hash
            && state(root)?
                .identities
                .contains_key(&article.meta.id.to_string())
        {
            article.meta.updated_at = Some(chrono::Utc::now().fixed_offset());
        }
    }
    let text = article_text(&article.meta, &article.body)?;
    let ledger = state(root)?;
    crate::render::check_identity(&s.config, &article, &ledger)?;
    atomic_write(&safe_path(&s.root, path)?, text.as_bytes())?;
    snapshot(root)
}
pub fn save_config(root: &Path, text: &str, expected: &str) -> Result<Snapshot> {
    let _lock = lock(root)?;
    let s = snapshot(root)?;
    s.expected(expected)?;
    let c: Publication = toml::from_str(text)?;
    validate_config(&c)?;
    let ledger = state(root)?;
    for a in s.articles().0 {
        crate::render::check_identity(&c, &a, &ledger)?;
    }
    atomic_write(&safe_path(&s.root, "publication.toml")?, text.as_bytes())?;
    snapshot(root)
}
pub fn delete_article(root: &Path, path: &str, expected: &str) -> Result<Snapshot> {
    let _lock = lock(root)?;
    let s = snapshot(root)?;
    s.expected(expected)?;
    let a = parse_article(path, s.files.get(path).context("Article not found")?)?;
    ensure!(
        a.meta.status == Status::Draft
            && !state(root)?.identities.contains_key(&a.meta.id.to_string()),
        "Only unpublished drafts can be deleted"
    );
    let trash = format!(".omapress/trash/{}.md", uuid::Uuid::new_v4());
    atomic_write(&safe_path(root, &trash)?, s.files.get(path).unwrap())?;
    fs::remove_file(safe_path(root, path)?)?;
    snapshot(root)
}
pub fn detect_image(bytes: &[u8]) -> Result<(&'static str, &'static str)> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Ok(("png", "image/png"));
    }
    if bytes.starts_with(b"\xff\xd8\xff") {
        return Ok(("jpg", "image/jpeg"));
    }
    if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        return Ok(("gif", "image/gif"));
    }
    if bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP") {
        return Ok(("webp", "image/webp"));
    }
    if bytes.get(4..8) == Some(b"ftyp")
        && [Some(b"avif".as_slice()), Some(b"avis".as_slice())].contains(&bytes.get(8..12))
    {
        return Ok(("avif", "image/avif"));
    }
    bail!("Unsupported image contents; use PNG, JPEG, WebP, GIF or AVIF")
}
pub fn import_media(root: &Path, source: &Path, expected: &str) -> Result<(Snapshot, String)> {
    let _lock = lock(root)?;
    let s = snapshot(root)?;
    s.expected(expected)?;
    let data = read_bounded(source, 32 * 1024 * 1024)?;
    let (ext, _) = detect_image(&data)?;
    let path = format!("media/{}.{}", hash(&data), ext);
    atomic_write(&safe_path(root, &path)?, &data)?;
    Ok((snapshot(root)?, format!("/{path}")))
}
