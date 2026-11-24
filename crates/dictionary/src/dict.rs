//! Leitura de dicionários livres: StarDict (.ifo/.idx/.dict[.dz]) e DICT
//! do dictd (.index/.dict[.dz]). Só o índice fica na memória; as definições
//! são lidas do disco na hora, e o .dz (dictzip) é aberto pedaço a pedaço.

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

pub struct Entry {
    pub word: String,
    /// Palavra sem acentos e em minúsculas (para a busca e a ordem).
    pub norm: String,
    pub off: u64,
    pub len: u32,
}

pub enum Format {
    /// StarDict: os tipos de cada definição (sametypesequence), se houver.
    StarDict(String),
    Dictd,
}

pub struct Dict {
    pub name: String,
    pub format: Format,
    pub entries: Vec<Entry>,
    data: Data,
}

enum Data {
    Plain(PathBuf),
    Dz(DictZip),
}

/// Tira acentos comuns e passa a minúsculas.
pub fn normalize(s: &str) -> String {
    s.chars()
        .flat_map(|c| c.to_lowercase())
        .map(|c| match c {
            'á' | 'à' | 'â' | 'ã' | 'ä' | 'å' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'í' | 'ì' | 'î' | 'ï' => 'i',
            'ó' | 'ò' | 'ô' | 'õ' | 'ö' => 'o',
            'ú' | 'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            'ñ' => 'n',
            c => c,
        })
        .collect()
}

/// Arquivo dictzip: gzip com a tabela dos pedaços no campo extra "RA".
struct DictZip {
    path: PathBuf,
    chunk_len: u64,
    /// Começo (no arquivo) de cada pedaço comprimido e o tamanho dele.
    chunks: Vec<(u64, u64)>,
    /// Sem a tabela: o arquivo inteiro descomprimido (só para os pequenos).
    whole: Option<Vec<u8>>,
}

impl DictZip {
    fn open(path: &Path) -> std::io::Result<Self> {
        let mut f = File::open(path)?;
        let mut head = [0u8; 10];
        f.read_exact(&mut head)?;
        let flags = head[3];
        let mut chunk_len = 0u64;
        let mut sizes: Vec<u64> = Vec::new();
        let mut pos = 10u64;
        if flags & 4 != 0 {
            let mut xl = [0u8; 2];
            f.read_exact(&mut xl)?;
            let xlen = u16::from_le_bytes(xl) as usize;
            let mut extra = vec![0u8; xlen];
            f.read_exact(&mut extra)?;
            pos += 2 + xlen as u64;
            let mut i = 0;
            while i + 4 <= extra.len() {
                let (si1, si2) = (extra[i], extra[i + 1]);
                let len = u16::from_le_bytes([extra[i + 2], extra[i + 3]]) as usize;
                let body = &extra[i + 4..(i + 4 + len).min(extra.len())];
                if si1 == b'R' && si2 == b'A' && body.len() >= 6 {
                    chunk_len = u16::from_le_bytes([body[2], body[3]]) as u64;
                    let count = u16::from_le_bytes([body[4], body[5]]) as usize;
                    sizes = (0..count).filter_map(|k| body.get(6 + 2 * k..8 + 2 * k)).map(|b| u16::from_le_bytes([b[0], b[1]]) as u64).collect();
                }
                i += 4 + len;
            }
        }
        // Nome e comentário terminam em zero; o CRC do cabeçalho tem 2 bytes.
        for flag in [8u8, 16] {
            if flags & flag != 0 {
                let mut b = [0u8; 1];
                loop {
                    f.read_exact(&mut b)?;
                    pos += 1;
                    if b[0] == 0 {
                        break;
                    }
                }
            }
        }
        if flags & 2 != 0 {
            pos += 2;
        }
        if chunk_len == 0 || sizes.is_empty() {
            let mut all = Vec::new();
            flate2::read::MultiGzDecoder::new(File::open(path)?).read_to_end(&mut all)?;
            return Ok(Self { path: path.to_path_buf(), chunk_len: 0, chunks: Vec::new(), whole: Some(all) });
        }
        let mut chunks = Vec::with_capacity(sizes.len());
        for s in sizes {
            chunks.push((pos, s));
            pos += s;
        }
        Ok(Self { path: path.to_path_buf(), chunk_len, chunks, whole: None })
    }

    fn read(&self, off: u64, len: u32) -> std::io::Result<Vec<u8>> {
        if let Some(all) = &self.whole {
            let a = (off as usize).min(all.len());
            let b = (a + len as usize).min(all.len());
            return Ok(all[a..b].to_vec());
        }
        let first = (off / self.chunk_len) as usize;
        let last = ((off + len as u64).saturating_sub(1) / self.chunk_len) as usize;
        let mut f = File::open(&self.path)?;
        let mut out = Vec::new();
        for i in first..=last.min(self.chunks.len().saturating_sub(1)) {
            let (start, size) = self.chunks[i];
            let mut comp = vec![0u8; size as usize];
            f.seek(SeekFrom::Start(start))?;
            f.read_exact(&mut comp)?;
            let mut d = flate2::Decompress::new(false);
            let mut buf = Vec::with_capacity(self.chunk_len as usize);
            d.decompress_vec(&comp, &mut buf, flate2::FlushDecompress::Sync).map_err(std::io::Error::other)?;
            out.extend_from_slice(&buf);
        }
        let skip = (off - first as u64 * self.chunk_len) as usize;
        let end = (skip + len as usize).min(out.len());
        Ok(out.get(skip.min(end)..end).unwrap_or(&[]).to_vec())
    }
}

fn b64(s: &str) -> u64 {
    s.bytes().fold(0u64, |acc, c| {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            _ => 63,
        };
        acc * 64 + v as u64
    })
}

fn data_for(base: &Path) -> Option<Data> {
    let dz = PathBuf::from(format!("{}.dict.dz", base.display()));
    let plain = PathBuf::from(format!("{}.dict", base.display()));
    if dz.exists() {
        DictZip::open(&dz).ok().map(Data::Dz)
    } else if plain.exists() {
        Some(Data::Plain(plain))
    } else {
        None
    }
}

impl Dict {
    /// Abre um StarDict pelo .ifo.
    pub fn stardict(ifo: &Path) -> Option<Self> {
        let info = std::fs::read_to_string(ifo).ok()?;
        let get = |k: &str| info.lines().find_map(|l| l.strip_prefix(k).and_then(|r| r.strip_prefix('='))).map(|v| v.trim().to_string());
        let base = ifo.with_extension("");
        let name = get("bookname").unwrap_or_else(|| base.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default());
        let bits64 = get("idxoffsetbits").as_deref() == Some("64");
        let idx_path = ifo.with_extension("idx");
        let raw = if idx_path.exists() {
            std::fs::read(&idx_path).ok()?
        } else {
            let mut v = Vec::new();
            flate2::read::GzDecoder::new(File::open(ifo.with_extension("idx.gz")).ok()?).read_to_end(&mut v).ok()?;
            v
        };
        let mut entries = Vec::new();
        let mut i = 0;
        let osz = if bits64 { 8 } else { 4 };
        while i < raw.len() {
            let Some(z) = raw[i..].iter().position(|b| *b == 0) else { break };
            let word = String::from_utf8_lossy(&raw[i..i + z]).into_owned();
            i += z + 1;
            if i + osz + 4 > raw.len() {
                break;
            }
            let off = if bits64 { u64::from_be_bytes(raw[i..i + 8].try_into().ok()?) } else { u32::from_be_bytes(raw[i..i + 4].try_into().ok()?) as u64 };
            let len = u32::from_be_bytes(raw[i + osz..i + osz + 4].try_into().ok()?);
            i += osz + 4;
            entries.push(Entry { norm: normalize(&word), word, off, len });
        }
        let mut d = Dict { name, format: Format::StarDict(get("sametypesequence").unwrap_or_default()), entries, data: data_for(&base)? };
        d.entries.sort_by(|a, b| a.norm.cmp(&b.norm));
        Some(d)
    }

    /// Abre um dicionário do dictd pelo .index.
    pub fn dictd(index: &Path) -> Option<Self> {
        let text = std::fs::read_to_string(index).ok()?;
        let base = index.with_extension("");
        let data = data_for(&base)?;
        let mut name = base.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let mut entries = Vec::new();
        let mut short = None;
        for l in text.lines() {
            let mut p = l.split('\t');
            let (Some(w), Some(o), Some(n)) = (p.next(), p.next(), p.next()) else { continue };
            if w == "00-database-short" || w == "00databaseshort" {
                short = Some((b64(o), b64(n) as u32));
            }
            if w.starts_with("00-database-") || w.starts_with("00database") {
                continue;
            }
            entries.push(Entry { word: w.to_string(), norm: normalize(w), off: b64(o), len: b64(n) as u32 });
        }
        let mut d = Dict { name: String::new(), format: Format::Dictd, entries, data };
        if let Some((o, n)) = short {
            if let Ok(b) = d.raw(o, n) {
                // A primeira linha é a própria palavra-chave; o nome vem depois.
                let t = String::from_utf8_lossy(&b).into_owned();
                if let Some(v) = t.lines().map(str::trim).find(|l| !l.is_empty() && !l.starts_with("00")) {
                    name = v.to_string();
                }
            }
        }
        d.name = name;
        d.entries.sort_by(|a, b| a.norm.cmp(&b.norm));
        Some(d)
    }

    fn raw(&self, off: u64, len: u32) -> std::io::Result<Vec<u8>> {
        match &self.data {
            Data::Plain(p) => {
                let mut f = File::open(p)?;
                f.seek(SeekFrom::Start(off))?;
                let mut b = vec![0u8; len as usize];
                let n = f.read(&mut b)?;
                b.truncate(n);
                Ok(b)
            }
            Data::Dz(z) => z.read(off, len),
        }
    }

    /// Índices das entradas que começam com `q` (já normalizada), em ordem.
    pub fn prefix(&self, q: &str, max: usize) -> Vec<usize> {
        let start = self.entries.partition_point(|e| e.norm.as_str() < q);
        (start..self.entries.len()).take_while(|i| self.entries[*i].norm.starts_with(q)).take(max).collect()
    }

    /// Texto da definição, sem marcação.
    pub fn definition(&self, i: usize) -> String {
        let e = &self.entries[i];
        let Ok(b) = self.raw(e.off, e.len) else { return String::new() };
        match &self.format {
            Format::Dictd => {
                let t = String::from_utf8_lossy(&b).into_owned();
                // O dictd repete a palavra na primeira linha.
                let mut lines: Vec<&str> = t.lines().collect();
                if lines.first().is_some_and(|l| normalize(l.trim()) == e.norm) {
                    lines.remove(0);
                }
                // Tira só o recuo comum a todas as linhas (o resto é estrutura).
                let common = lines.iter().filter(|l| !l.trim().is_empty()).map(|l| l.len() - l.trim_start().len()).min().unwrap_or(0);
                lines.iter().map(|l| l.get(common..).unwrap_or("").trim_end()).collect::<Vec<_>>().join("\n").trim_matches('\n').to_string()
            }
            Format::StarDict(seq) => stardict_text(&b, seq),
        }
    }
}

/// Junta os campos de uma definição StarDict, tirando HTML/XDXF/Pango.
fn stardict_text(b: &[u8], seq: &str) -> String {
    let mut parts: Vec<String> = Vec::new();
    let mut i = 0;
    let types: Vec<u8> = seq.bytes().collect();
    let mut k = 0;
    while i < b.len() {
        // Com sametypesequence os tipos não vêm no dado; senão, 1 byte antes de cada campo.
        let t = if types.is_empty() {
            let t = b[i];
            i += 1;
            t
        } else if k < types.len() {
            types[k]
        } else {
            break;
        };
        let last = !types.is_empty() && k + 1 == types.len();
        k += 1;
        if t.is_ascii_lowercase() {
            let end = if last { b.len() } else { b[i..].iter().position(|c| *c == 0).map(|z| i + z).unwrap_or(b.len()) };
            let s = String::from_utf8_lossy(&b[i..end]).into_owned();
            i = end + 1;
            parts.push(match t {
                b'h' | b'g' | b'x' | b'k' => strip_tags(&s),
                _ => s,
            });
        } else {
            // Campos binários (som, imagem): pula.
            let size = if last { b.len() - i } else { b.get(i..i + 4).map(|x| u32::from_be_bytes([x[0], x[1], x[2], x[3]]) as usize).unwrap_or(0) + 4 };
            i += size;
        }
    }
    parts.join("\n").trim().to_string()
}

/// Tira as etiquetas de HTML; <br> e fim de parágrafo viram quebra de linha.
pub fn strip_tags(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut tag = String::new();
    let mut in_tag = false;
    for c in s.chars() {
        match (in_tag, c) {
            (false, '<') => {
                in_tag = true;
                tag.clear();
            }
            (true, '>') => {
                in_tag = false;
                let t = tag.trim_start_matches('/').split([' ', '/']).next().unwrap_or("").to_lowercase();
                if matches!(t.as_str(), "br" | "p" | "div" | "li" | "tr" | "def" | "ex") && !out.ends_with('\n') {
                    out.push('\n');
                }
            }
            (true, c) => tag.push(c),
            (false, c) => out.push(c),
        }
    }
    out.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&apos;", "'").replace("&nbsp;", " ").replace("&amp;", "&")
}

/// Pastas onde procurar dicionários.
pub fn dirs() -> Vec<PathBuf> {
    let home = PathBuf::from(std::env::var_os("HOME").unwrap_or_default());
    let data = std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).unwrap_or_else(|| home.join(".local/share"));
    vec![data.join("glove/dicionarios"), home.join(".stardict/dic"), PathBuf::from("/usr/share/stardict/dic"), PathBuf::from("/usr/share/dictd")]
}

/// Todos os dicionários nas pastas (busca em subpastas, um nível).
pub fn load_all(dirs: &[PathBuf]) -> Vec<Dict> {
    let mut files = Vec::new();
    for d in dirs {
        for e in std::fs::read_dir(d).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() {
                files.extend(std::fs::read_dir(&p).into_iter().flatten().flatten().map(|e| e.path()));
            } else {
                files.push(p);
            }
        }
    }
    files.sort();
    let mut out = Vec::new();
    for p in files {
        let ext = p.extension().map(|e| e.to_string_lossy().into_owned()).unwrap_or_default();
        let d = match ext.as_str() {
            "ifo" => Dict::stardict(&p),
            "index" => Dict::dictd(&p),
            _ => None,
        };
        out.extend(d);
    }
    out
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use std::io::Write;

    /// Escreve um dictzip com pedaços de `chunk` bytes.
    pub fn write_dz(path: &Path, data: &[u8], chunk: usize) {
        let mut sizes = Vec::new();
        let mut body = Vec::new();
        let mut crc = flate2::Crc::new();
        crc.update(data);
        let mut c = flate2::Compress::new(flate2::Compression::default(), false);
        let pieces: Vec<&[u8]> = data.chunks(chunk).collect();
        for (k, piece) in pieces.iter().enumerate() {
            let before = body.len();
            let flush = if k + 1 == pieces.len() { flate2::FlushCompress::Finish } else { flate2::FlushCompress::Full };
            let mut out = Vec::with_capacity(piece.len() * 2 + 64);
            c.compress_vec(piece, &mut out, flush).unwrap();
            body.extend_from_slice(&out);
            sizes.push((body.len() - before) as u16);
        }
        let mut extra = vec![b'R', b'A'];
        let ra_len = 6 + 2 * sizes.len();
        extra.extend_from_slice(&(ra_len as u16).to_le_bytes());
        extra.extend_from_slice(&1u16.to_le_bytes());
        extra.extend_from_slice(&(chunk as u16).to_le_bytes());
        extra.extend_from_slice(&(sizes.len() as u16).to_le_bytes());
        for s in &sizes {
            extra.extend_from_slice(&s.to_le_bytes());
        }
        let mut f = File::create(path).unwrap();
        f.write_all(&[0x1f, 0x8b, 8, 4, 0, 0, 0, 0, 0, 3]).unwrap();
        f.write_all(&(extra.len() as u16).to_le_bytes()).unwrap();
        f.write_all(&extra).unwrap();
        f.write_all(&body).unwrap();
        f.write_all(&crc.sum().to_le_bytes()).unwrap();
        f.write_all(&(data.len() as u32).to_le_bytes()).unwrap();
    }

    /// Cria um StarDict pequeno em `dir`, com dictzip.
    pub fn make_stardict(dir: &Path, words: &[(&str, &str)]) {
        std::fs::create_dir_all(dir).unwrap();
        let mut data = Vec::new();
        let mut idx = Vec::new();
        let mut sorted: Vec<&(&str, &str)> = words.iter().collect();
        sorted.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));
        for (w, d) in sorted {
            let off = data.len() as u32;
            data.extend_from_slice(d.as_bytes());
            idx.extend_from_slice(w.as_bytes());
            idx.push(0);
            idx.extend_from_slice(&off.to_be_bytes());
            idx.extend_from_slice(&(d.len() as u32).to_be_bytes());
        }
        std::fs::write(dir.join("teste.ifo"), format!("StarDict's dict ifo file\nversion=2.4.2\nwordcount={}\nidxfilesize={}\nbookname=Dicionário de Teste\nsametypesequence=h\n", words.len(), idx.len())).unwrap();
        std::fs::write(dir.join("teste.idx"), &idx).unwrap();
        write_dz(&dir.join("teste.dict.dz"), &data, 64);
    }

    #[test]
    fn stardict_com_dictzip() {
        let dir = std::env::temp_dir().join(format!("glove-dic-teste-{}", std::process::id()));
        let long = "uma definição longa ".repeat(20);
        make_stardict(&dir, &[("casa", "<b>casa</b><br>lugar onde se mora"), ("água", "líquido &amp; vida"), ("casaco", &long), ("zebra", "animal listrado")]);
        let d = Dict::stardict(&dir.join("teste.ifo")).expect("abre");
        assert_eq!(d.name, "Dicionário de Teste");
        let hits = d.prefix("cas", 10);
        assert_eq!(hits.iter().map(|i| d.entries[*i].word.as_str()).collect::<Vec<_>>(), ["casa", "casaco"]);
        assert_eq!(d.definition(hits[0]), "casa\nlugar onde se mora");
        // Atravessa vários pedaços do dictzip.
        assert_eq!(d.definition(hits[1]), long.trim());
        let a = d.prefix("agu", 10);
        assert_eq!(d.definition(a[0]), "líquido & vida");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn dictd_simples() {
        let dir = std::env::temp_dir().join(format!("glove-dictd-teste-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let short = "00-database-short\n    Teste DICT\n";
        let gato = "gato\n   felino doméstico\n";
        let data = format!("{short}{gato}");
        std::fs::write(dir.join("t.dict"), &data).unwrap();
        let enc = |mut n: u64| {
            let a = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
            let mut s = Vec::new();
            loop {
                s.push(a[(n % 64) as usize]);
                n /= 64;
                if n == 0 {
                    break;
                }
            }
            s.reverse();
            String::from_utf8(s).unwrap()
        };
        std::fs::write(dir.join("t.index"), format!("00-database-short\t{}\t{}\ngato\t{}\t{}\n", enc(0), enc(short.len() as u64), enc(short.len() as u64), enc(gato.len() as u64))).unwrap();
        let d = Dict::dictd(&dir.join("t.index")).expect("abre");
        assert_eq!(d.name, "Teste DICT");
        assert_eq!(d.entries.len(), 1);
        assert_eq!(d.definition(0), "felino doméstico");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
