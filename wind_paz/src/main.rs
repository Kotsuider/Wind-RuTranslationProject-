use std::collections::HashMap;
use std::env;
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

const P_INIT: [u32; 18] = [
    0x243F6A88, 0x85A308D3, 0x13198A2E, 0x03707344, 0xA4093822, 0x299F31D0,
    0x082EFA98, 0xEC4E6C89, 0x452821E6, 0x38D01377, 0xBE5466CF, 0x34E90C6C,
    0xC0AC29B7, 0xC97C50DD, 0x3F84D5B5, 0xB5470917, 0x9216D5D9, 0x8979FB1B,
];

pub struct MusicaBlowfish {
    p: [u32; 18],
    s: [[u32; 256]; 4],
}

impl MusicaBlowfish {
    pub fn new(raw_key: &[u8], s_init: &[[u32; 256]; 4]) -> Self {
        let key: Vec<u8> = raw_key.iter().map(|&b| b.wrapping_neg()).collect();
        let mut p = P_INIT;
        let mut s = *s_init;

        let key_len = key.len();
        let mut key_pos = 0;
        for i in 0..18 {
            let mut data = 0u32;
            for _ in 0..4 {
                data = (data << 8) | (key[key_pos] as u32);
                key_pos = (key_pos + 1) % key_len;
            }
            p[i] ^= data;
        }

        let mut datal = 0u32;
        let mut datar = 0u32;
        for i in (0..18).step_by(2) {
            let (l, r) = Self::encipher_block_raw(datal, datar, &p, &s);
            datal = l;
            datar = r;
            p[i] = datal;
            p[i + 1] = datar;
        }

        for s_idx in 0..4 {
            for i in (0..256).step_by(2) {
                let (l, r) = Self::encipher_block_raw(datal, datar, &p, &s);
                datal = l;
                datar = r;
                s[s_idx][i] = datal;
                s[s_idx][i + 1] = datar;
            }
        }

        MusicaBlowfish { p, s }
    }

    #[inline(always)]
    fn encipher_block_raw(mut xl: u32, mut xr: u32, p: &[u32; 18], s: &[[u32; 256]; 4]) -> (u32, u32) {
        for i in 0..16 {
            xl ^= p[i];
            let a = ((xl >> 24) & 0xff) as usize;
            let b = ((xl >> 16) & 0xff) as usize;
            let c = ((xl >> 8) & 0xff) as usize;
            let d = (xl & 0xff) as usize;
            let f = (s[0][a].wrapping_add(s[1][b]) ^ s[2][c]).wrapping_add(s[3][d]);
            xr ^= f;
            std::mem::swap(&mut xl, &mut xr);
        }
        std::mem::swap(&mut xl, &mut xr);
        xr ^= p[16];
        xl ^= p[17];
        (xl, xr)
    }

    #[inline(always)]
    pub fn encipher_block(&self, xl: u32, xr: u32) -> (u32, u32) {
        Self::encipher_block_raw(xl, xr, &self.p, &self.s)
    }

    #[inline(always)]
    pub fn decipher_block(&self, mut xl: u32, mut xr: u32) -> (u32, u32) {
        for i in (2..=17).rev() {
            xl ^= self.p[i];
            let a = ((xl >> 24) & 0xff) as usize;
            let b = ((xl >> 16) & 0xff) as usize;
            let c = ((xl >> 8) & 0xff) as usize;
            let d = (xl & 0xff) as usize;
            let f = (self.s[0][a].wrapping_add(self.s[1][b]) ^ self.s[2][c]).wrapping_add(self.s[3][d]);
            xr ^= f;
            std::mem::swap(&mut xl, &mut xr);
        }
        std::mem::swap(&mut xl, &mut xr);
        xr ^= self.p[1];
        xl ^= self.p[0];
        (xl, xr)
    }

    pub fn decrypt(&self, data: &[u8]) -> Vec<u8> {
        let mut out = Vec::with_capacity(data.len());
        for chunk in data.chunks_exact(8) {
            let xl = u32::from_le_bytes(chunk[0..4].try_into().unwrap());
            let xr = u32::from_le_bytes(chunk[4..8].try_into().unwrap());
            let (ol, or) = self.decipher_block(xl, xr);
            out.extend_from_slice(&ol.to_le_bytes());
            out.extend_from_slice(&or.to_le_bytes());
        }
        out
    }

    pub fn encrypt(&self, data: &[u8]) -> Vec<u8> {
        assert_eq!(data.len() % 8, 0, "Data size must be multiple of 8");
        let mut out = Vec::with_capacity(data.len());
        for chunk in data.chunks_exact(8) {
            let xl = u32::from_le_bytes(chunk[0..4].try_into().unwrap());
            let xr = u32::from_le_bytes(chunk[4..8].try_into().unwrap());
            let (ol, or) = self.encipher_block(xl, xr);
            out.extend_from_slice(&ol.to_le_bytes());
            out.extend_from_slice(&or.to_le_bytes());
        }
        out
    }
}

fn load_s_boxes() -> [[u32; 256]; 4] {
    let mut exe = File::open("r:\\Wind\\WindRP_unpacked.exe").expect("WindRP_unpacked.exe not found");
    let mut data = vec![0u8; 946176];
    exe.read_exact(&mut data).expect("Failed to read exe");

    let mut boxes = [[0u32; 256]; 4];
    let offset = 0x9AD80;
    for s in 0..4 {
        for i in 0..256 {
            let idx = offset + (s * 256 + i) * 4;
            boxes[s][i] = u32::from_le_bytes(data[idx..idx + 4].try_into().unwrap());
        }
    }
    boxes
}

fn load_keys_from_exe() -> Vec<[u8; 32]> {
    let mut exe = File::open("r:\\Wind\\WindRP_unpacked.exe").expect("WindRP_unpacked.exe not found");
    let mut data = vec![0u8; 946176];
    exe.read_exact(&mut data).expect("Failed to read exe");

    let mut keys = Vec::new();
    let offset = 0xB6288;
    for i in 0..18 {
        let start = offset + i * 32;
        let mut key = [0u8; 32];
        key.copy_from_slice(&data[start..start + 32]);
        keys.push(key);
    }
    keys
}

fn get_archive_type_index(name_or_path: &Path) -> usize {
    let stem = name_or_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    let prefix = stem.trim_end_matches(|c: char| c.is_ascii_digit());
    if prefix.contains("scr") {
        0
    } else if prefix.contains("bgm") {
        4
    } else if prefix.contains("bg") {
        1
    } else if prefix.contains("st") {
        2
    } else if prefix.contains("sys") {
        3
    } else if prefix.contains("voice") {
        5
    } else if prefix.contains("se") {
        6
    } else if prefix.contains("mov") {
        8
    } else {
        panic!("Unknown archive type for: {:?}", name_or_path);
    }
}

struct PazEntry {
    name: String,
    offset: u64,
    #[allow(dead_code)]
    unpacked_size: u32,
    size: u32,
    aligned_size: u32,
    is_packed: bool,
}

fn unpack_paz(paz_path: &Path, s_init: &[[u32; 256]; 4], keys: &[[u8; 32]]) {
    println!("\n[*] Распаковка архива: {:?}", paz_path);
    let type_idx = get_archive_type_index(paz_path);
    let index_key = &keys[type_idx];
    let data_key = &keys[type_idx + 9];

    let bf_index = MusicaBlowfish::new(index_key, s_init);
    let bf_data = MusicaBlowfish::new(data_key, s_init);

    let stem = paz_path.file_stem().unwrap().to_str().unwrap();
    let out_dir = paz_path.parent().unwrap().join(format!("unpacked_{}", stem));
    fs::create_dir_all(&out_dir).unwrap();

    let mut f = File::open(paz_path).expect("Failed to open paz file");
    let mut hdr = [0u8; 4];
    f.read_exact(&mut hdr).expect("Failed to read header");

    let mut index_size = u32::from_le_bytes(hdr);
    let xor_key = (index_size >> 24) as u8;
    if xor_key != 0 {
        let mask = (xor_key as u32) * 0x01010101;
        index_size ^= mask;
    }

    let mut raw_index = vec![0u8; index_size as usize];
    f.read_exact(&mut raw_index).expect("Failed to read index");
    if xor_key != 0 {
        for b in raw_index.iter_mut() {
            *b ^= xor_key;
        }
    }

    let dec_index = bf_index.decrypt(&raw_index);
    let count = u32::from_le_bytes(dec_index[0..4].try_into().unwrap()) as usize;
    println!("[*] Найдено файлов в индексе: {}", count);

    let mut pos = 4;
    let mut entries = Vec::with_capacity(count);

    for _ in 0..count {
        let null_pos = dec_index[pos..]
            .iter()
            .position(|&b| b == 0)
            .map(|p| pos + p)
            .expect("Null terminator not found");

        let raw_name = &dec_index[pos..null_pos];
        let (cow, _, _) = encoding_rs::SHIFT_JIS.decode(raw_name);
        let name_clean = cow.into_owned();

        pos = null_pos + 1;
        let offset = u64::from_le_bytes(dec_index[pos..pos + 8].try_into().unwrap());
        let unpacked_size = u32::from_le_bytes(dec_index[pos + 8..pos + 12].try_into().unwrap());
        let size = u32::from_le_bytes(dec_index[pos + 12..pos + 16].try_into().unwrap());
        let aligned_size = u32::from_le_bytes(dec_index[pos + 16..pos + 20].try_into().unwrap());
        let is_packed = u32::from_le_bytes(dec_index[pos + 20..pos + 24].try_into().unwrap()) != 0;
        pos += 24;

        entries.push(PazEntry {
            name: name_clean,
            offset,
            unpacked_size,
            size,
            aligned_size,
            is_packed,
        });
    }

    let total = entries.len();
    for (idx, entry) in entries.into_iter().enumerate() {
        f.seek(SeekFrom::Start(entry.offset)).expect("Seek error");
        let mut enc_data = vec![0u8; entry.aligned_size as usize];
        f.read_exact(&mut enc_data).expect("Read file error");

        if xor_key != 0 {
            for b in enc_data.iter_mut() {
                *b ^= xor_key;
            }
        }

        let dec_data = bf_data.decrypt(&enc_data);
        let valid_data = &dec_data[..entry.size as usize];

        let final_data = if entry.is_packed {
            match miniz_oxide::inflate::decompress_to_vec_zlib(valid_data) {
                Ok(decompressed) => decompressed,
                Err(err) => {
                    eprintln!("  [!] Ошибка zlib разжатия {}: {:?}", entry.name, err);
                    valid_data.to_vec()
                }
            }
        } else {
            valid_data.to_vec()
        };

        let file_path = out_dir.join(&entry.name);
        if let Some(p) = file_path.parent() {
            fs::create_dir_all(p).unwrap();
        }
        let mut out_f = File::create(&file_path).expect("Failed to create file");
        out_f.write_all(&final_data).expect("Failed to write data");

        if idx < 5 || idx == total - 1 || idx % 50 == 0 {
            println!("  [{}/{}] Извлечен: {} ({} байт)", idx + 1, total, entry.name, final_data.len());
        }
    }
    println!("[+] Успешно распакован в: {:?}", out_dir);
}

struct PackedFileInfo {
    sjis_name: Vec<u8>,
    unpacked_size: u32,
    size: u32,
    aligned_size: u32,
    is_packed: bool,
    encrypted_bytes: Vec<u8>,
}

fn pack_paz(src_dir: &Path, dst_paz: &Path, s_init: &[[u32; 256]; 4], keys: &[[u8; 32]]) {
    println!("\n[*] Запаковка каталога {:?} в архив {:?}", src_dir, dst_paz);
    let type_idx = get_archive_type_index(dst_paz);
    let index_key = &keys[type_idx];
    let data_key = &keys[type_idx + 9];

    let bf_index = MusicaBlowfish::new(index_key, s_init);
    let bf_data = MusicaBlowfish::new(data_key, s_init);

    let mut file_paths = Vec::new();
    collect_files_recursive(src_dir, src_dir, &mut file_paths);
    file_paths.sort();

    println!("[*] Найдено файлов для запаковки: {}", file_paths.len());

    let mut packed_files: Vec<PackedFileInfo> = Vec::with_capacity(file_paths.len());

    for (idx, (rel_name, full_path)) in file_paths.iter().enumerate() {
        let mut raw_data = Vec::new();
        File::open(full_path)
            .unwrap_or_else(|e| panic!("Не удалось открыть {:?}: {}", full_path, e))
            .read_to_end(&mut raw_data)
            .unwrap();

        let unpacked_size = raw_data.len() as u32;

        let compressed = miniz_oxide::deflate::compress_to_vec_zlib(&raw_data, 6);
        let (data_to_encrypt, is_packed) = if compressed.len() < raw_data.len() {
            (compressed, true)
        } else {
            (raw_data, false)
        };

        let size = data_to_encrypt.len() as u32;
        let aligned_size = ((size + 7) / 8) * 8;
        let mut padded = data_to_encrypt;
        padded.resize(aligned_size as usize, 0);

        let encrypted = bf_data.encrypt(&padded);

        let (sjis_bytes, _, _) = encoding_rs::SHIFT_JIS.encode(rel_name);

        packed_files.push(PackedFileInfo {
            sjis_name: sjis_bytes.into_owned(),
            unpacked_size,
            size,
            aligned_size,
            is_packed,
            encrypted_bytes: encrypted,
        });

        if idx < 5 || idx == file_paths.len() - 1 || idx % 50 == 0 {
            println!("  [{}/{}] Подготовлен: {} (исходный: {} байт, сжат: {} байт)",
                idx + 1, file_paths.len(), rel_name, unpacked_size, size);
        }
    }

    let count = packed_files.len() as u32;
    let mut raw_index_len = 4usize;
    for pf in &packed_files {
        raw_index_len += pf.sjis_name.len() + 1 + 24;
    }
    let aligned_index_len = ((raw_index_len + 7) / 8) * 8;
    let data_start_offset = (4 + aligned_index_len) as u64;

    let mut cur_offset = data_start_offset;
    let mut raw_index = Vec::with_capacity(aligned_index_len);
    raw_index.extend_from_slice(&count.to_le_bytes());

    for pf in &packed_files {
        raw_index.extend_from_slice(&pf.sjis_name);
        raw_index.push(0);

        raw_index.extend_from_slice(&cur_offset.to_le_bytes());
        raw_index.extend_from_slice(&pf.unpacked_size.to_le_bytes());
        raw_index.extend_from_slice(&pf.size.to_le_bytes());
        raw_index.extend_from_slice(&pf.aligned_size.to_le_bytes());
        raw_index.extend_from_slice(&(if pf.is_packed { 1u32 } else { 0u32 }).to_le_bytes());

        cur_offset += pf.aligned_size as u64;
    }

    raw_index.resize(aligned_index_len, 0);
    let enc_index = bf_index.encrypt(&raw_index);

    let mut out_f = File::create(dst_paz)
        .unwrap_or_else(|e| panic!("Не удалось создать выходной файл {:?}: {}", dst_paz, e));

    let index_size_hdr = aligned_index_len as u32;
    out_f.write_all(&index_size_hdr.to_le_bytes()).unwrap();
    out_f.write_all(&enc_index).unwrap();

    for pf in &packed_files {
        out_f.write_all(&pf.encrypted_bytes).unwrap();
    }

    println!("[+] Архив успешно создан: {:?} (размер: {} байт)", dst_paz, out_f.metadata().unwrap().len());
}

fn collect_files_recursive(base_dir: &Path, cur_dir: &Path, out: &mut Vec<(String, PathBuf)>) {
    if let Ok(entries) = fs::read_dir(cur_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_files_recursive(base_dir, &path, out);
            } else if path.is_file() {
                let rel = path.strip_prefix(base_dir).unwrap();
                let rel_str = rel.to_str().unwrap().replace('/', "\\");
                out.push((rel_str, path));
            }
        }
    }
}

// ---------------------- RU_F Translation & Word Wrap ----------------------

fn apply_ru_f(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        let mapped = match ch {
            // Special mappings (priority)
            'Ъ' => '[',
            'Ь' => ']',
            'ё' => '`',
            'э' => ';',
            'ы' => '|',
            'я' => '/',
            'Ы' => '\u{00A1}',
            'ь' => '&',
            'ъ' => '+',
            'Ю' => '%',
            'ю' => '$',
            '—' => '#',
            'Я' => '>',
            'Ё' => '<',
            '«' => '^',
            '»' => '@',
            'Э' => '=',
            'Й' => 'J',
            'й' => 'j',

            // Uppercase cyrillic -> Latin
            'А' => 'A', 'Б' => 'B', 'В' => 'C', 'Г' => 'D', 'Д' => 'E', 'Е' => 'F',
            'Ж' => 'G', 'З' => 'H', 'И' => 'I', 'К' => 'K', 'Л' => 'L',
            'М' => 'M', 'Н' => 'N', 'О' => 'O', 'П' => 'P', 'Р' => 'Q', 'С' => 'R',
            'Т' => 'S', 'У' => 'T', 'Ф' => 'U', 'Х' => 'V', 'Ц' => 'W', 'Ч' => 'X',
            'Ш' => 'Y', 'Щ' => 'Z',

            // Lowercase cyrillic -> Latin
            'а' => 'a', 'б' => 'b', 'в' => 'c', 'г' => 'd', 'д' => 'e', 'е' => 'f',
            'ж' => 'g', 'з' => 'h', 'и' => 'i', 'к' => 'k', 'л' => 'l',
            'м' => 'm', 'н' => 'n', 'о' => 'o', 'п' => 'p', 'р' => 'q', 'с' => 'r',
            'т' => 's', 'у' => 't', 'ф' => 'u', 'х' => 'v', 'ц' => 'w', 'ч' => 'x',
            'ш' => 'y', 'щ' => 'z',

            _ => ch,
        };
        out.push(mapped);
    }
    out
}

fn wrap_text(text: &str, max_len: usize) -> String {
    let clean = text.replace('\r', "");
    let paragraphs: Vec<&str> = if clean.contains("\\n") {
        clean.split("\\n").collect()
    } else if clean.contains('\n') {
        clean.split('\n').collect()
    } else {
        vec![&clean]
    };

    let mut result_paragraphs = Vec::new();

    for p in paragraphs {
        let p_trimmed = p.trim();
        if p_trimmed.is_empty() {
            continue;
        }

        let words: Vec<&str> = p_trimmed.split_whitespace().collect();
        if words.is_empty() {
            continue;
        }

        let mut current_line = String::new();
        let mut current_len = 0usize;
        let mut lines = Vec::new();

        for word in words {
            let word_char_count = word.chars().count();
            if current_len == 0 {
                current_line.push_str(word);
                current_len = word_char_count;
            } else if current_len + 1 + word_char_count <= max_len {
                current_line.push(' ');
                current_line.push_str(word);
                current_len += 1 + word_char_count;
            } else {
                lines.push(current_line);
                current_line = word.to_string();
                current_len = word_char_count;
            }
        }
        if !current_line.is_empty() {
            lines.push(current_line);
        }

        result_paragraphs.push(lines.join("\\n"));
    }

    result_paragraphs.join("\\n")
}

// ---------------------- Markdown Export & Import ----------------------

fn decode_sjis_lossy(bytes: &[u8]) -> String {
    let (cow, _, _) = encoding_rs::SHIFT_JIS.decode(bytes);
    cow.into_owned()
}

fn read_lines_sjis(path: &Path) -> Vec<String> {
    let mut data = Vec::new();
    if let Ok(mut f) = File::open(path) {
        let _ = f.read_to_end(&mut data);
    }
    let decoded = decode_sjis_lossy(&data);
    decoded.lines().map(|s| s.to_string()).collect()
}

struct ParsedMessage {
    speaker: String,
    text: String,
}

fn parse_mis_line(line: &str) -> Option<(&str, &str, &str, &str)> {
    if !line.starts_with(".message") {
        return None;
    }
    let rest = line[8..].trim_start();
    let id_end = rest.find(|c: char| c.is_whitespace())?;
    let id = &rest[..id_end];
    let rest = rest[id_end..].trim_start();

    let voice_end = rest.find(|c: char| c.is_whitespace())?;
    let voice = &rest[..voice_end];
    let rest = rest[voice_end..].trim_start();

    let spk_end = rest.find(|c: char| c.is_whitespace())?;
    let speaker = &rest[..spk_end];
    let text = rest[spk_end..].trim_start_matches(['\t', ' ']);

    Some((id, voice, speaker, text))
}

fn parse_mis_messages(lines: &[String]) -> HashMap<String, ParsedMessage> {
    let mut map = HashMap::new();
    for line in lines {
        if let Some((id, _, speaker, text)) = parse_mis_line(line) {
            map.insert(
                id.to_string(),
                ParsedMessage {
                    speaker: speaker.to_string(),
                    text: text.to_string(),
                },
            );
        }
    }
    map
}

fn escape_md_cell(s: &str) -> String {
    s.replace('|', "\\|").replace('\r', "").replace('\n', "<br>")
}

fn unescape_md_cell(s: &str) -> String {
    s.replace("<br>", "\n").replace("\\|", "|")
}

fn export_md(en_dir: &Path, jp1_dir: &Path, jp0_dir: &Path, out_dir: &Path) {
    println!("\n[*] Экспорт скриптов в Markdown таблицы...");
    println!("    EN (scr2): {:?}", en_dir);
    println!("    JP patch (scr1): {:?}", jp1_dir);
    println!("    JP base (scr): {:?}", jp0_dir);
    println!("    Выходная папка: {:?}", out_dir);

    fs::create_dir_all(out_dir).unwrap();

    let mut en_files = Vec::new();
    collect_files_recursive(en_dir, en_dir, &mut en_files);
    en_files.sort();

    let mut total_exported_files = 0;
    let mut total_lines = 0;

    for (rel_name, en_path) in en_files {
        if !rel_name.ends_with(".mis") && !rel_name.ends_with(".sc") {
            continue;
        }

        let jp_path1 = jp1_dir.join(&rel_name);
        let jp_path0 = jp0_dir.join(&rel_name);
        let jp_lines = if jp_path1.exists() {
            read_lines_sjis(&jp_path1)
        } else if jp_path0.exists() {
            read_lines_sjis(&jp_path0)
        } else {
            Vec::new()
        };

        let jp_messages = parse_mis_messages(&jp_lines);
        let en_lines = read_lines_sjis(&en_path);

        let mut md_content = String::new();
        md_content.push_str(&format!("# Script: {}\n\n", rel_name));
        md_content.push_str("| ID | Ch | Original (JP) | TLE (EN) | TL (RU) |\n");
        md_content.push_str("| :--- | :--- | :--- | :--- | :--- |\n");

        let mut count_in_file = 0;

        for line in &en_lines {
            if let Some((id, _, speaker, en_text)) = parse_mis_line(line) {
                let jp_text = jp_messages.get(id).map(|m| m.text.as_str()).unwrap_or("");
                let jp_speaker = jp_messages.get(id).map(|m| m.speaker.as_str()).unwrap_or(speaker);

                let ch_display = if speaker != "*" {
                    speaker
                } else if jp_speaker != "*" {
                    jp_speaker
                } else {
                    ""
                };

                md_content.push_str(&format!(
                    "| {} | {} | {} | {} |  |\n",
                    id,
                    escape_md_cell(ch_display),
                    escape_md_cell(jp_text),
                    escape_md_cell(en_text),
                ));
                count_in_file += 1;
            } else if line.starts_with(".select\t") || line.starts_with(".select ") {
                let parts: Vec<&str> = line.split('\t').collect();
                for (opt_idx, opt) in parts[1..].iter().enumerate() {
                    let opt_trimmed = opt.trim();
                    if opt_trimmed.is_empty() { continue; }
                    let (opt_text, _) = if let Some(colon) = opt_trimmed.rfind(':') {
                        (&opt_trimmed[..colon], &opt_trimmed[colon+1..])
                    } else {
                        (opt_trimmed, "")
                    };

                    md_content.push_str(&format!(
                        "| SELECT_{} |  |  | {} |  |\n",
                        opt_idx + 1,
                        escape_md_cell(opt_text),
                    ));
                    count_in_file += 1;
                }
            }
        }

        if count_in_file > 0 {
            let md_filename = format!("{}.md", rel_name);
            let md_path = out_dir.join(&md_filename);
            let mut out_f = File::create(&md_path).unwrap();
            out_f.write_all(md_content.as_bytes()).unwrap();

            total_exported_files += 1;
            total_lines += count_in_file;
            println!("  [+] Экспортирован: {} ({} строк) -> {:?}", rel_name, count_in_file, md_path.file_name().unwrap());
        }
    }

    println!("\n[+] Экспорт завершен! Экспортировано {} файлов, всего {} диалоговых строк.", total_exported_files, total_lines);
}

fn parse_md_table(md_content: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for line in md_content.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with('|') || !trimmed.ends_with('|') {
            continue;
        }
        let parts: Vec<&str> = trimmed.split('|').collect();
        // Parts: ["", ID, Ch, JP, EN, RU, ""]
        if parts.len() >= 7 {
            let id = parts[1].trim();
            if id.is_empty() || id == "ID" || id.starts_with(':') || id.starts_with("---") {
                continue;
            }
            let ru_cell = parts[5].trim();
            if !ru_cell.is_empty() {
                let unescaped = unescape_md_cell(ru_cell);
                map.insert(id.to_string(), unescaped);
            }
        }
    }
    map
}

fn import_md(md_dir: &Path, scr_template_dir: &Path, out_scr_dir: &Path, wrap_limit: usize) {
    println!("\n[*] Импорт переводов из Markdown в скрипты...");
    println!("    MD папка: {:?}", md_dir);
    println!("    Шаблон скриптов: {:?}", scr_template_dir);
    println!("    Выходная папка скриптов: {:?}", out_scr_dir);
    println!("    Лимит переноса строки: {} символов", wrap_limit);

    fs::create_dir_all(out_scr_dir).unwrap();

    let mut md_files = Vec::new();
    collect_files_recursive(md_dir, md_dir, &mut md_files);
    md_files.sort();

    let mut total_imported_lines = 0;
    let mut total_files = 0;

    for (rel_name, md_path) in md_files {
        if !rel_name.ends_with(".md") {
            continue;
        }

        let script_name = &rel_name[..rel_name.len() - 3]; // strip .md
        let template_script_path = scr_template_dir.join(script_name);
        if !template_script_path.exists() {
            eprintln!("  [!] Шаблон скрипта не найден для: {:?}", template_script_path);
            continue;
        }

        let mut md_bytes = Vec::new();
        File::open(&md_path).unwrap().read_to_end(&mut md_bytes).unwrap();
        let md_text = String::from_utf8_lossy(&md_bytes);
        let translations = parse_md_table(&md_text);

        if translations.is_empty() {
            continue;
        }

        let template_lines = read_lines_sjis(&template_script_path);
        let mut new_lines = Vec::with_capacity(template_lines.len());
        let mut file_imported_count = 0;

        for line in &template_lines {
            if let Some((id, voice, speaker, _orig_text)) = parse_mis_line(line) {
                if let Some(ru_text) = translations.get(id) {
                    let wrapped = wrap_text(ru_text, wrap_limit);
                    let ruf_converted = apply_ru_f(&wrapped);
                    new_lines.push(format!(".message\t{}\t{}\t{}\t{}", id, voice, speaker, ruf_converted));
                    file_imported_count += 1;
                } else {
                    new_lines.push(line.clone());
                }
            } else if line.starts_with(".select\t") || line.starts_with(".select ") {
                let parts: Vec<&str> = line.split('\t').collect();
                let mut new_parts = vec![parts[0].to_string()];
                for (opt_idx, opt) in parts[1..].iter().enumerate() {
                    let opt_trimmed = opt.trim();
                    if opt_trimmed.is_empty() {
                        new_parts.push(String::new());
                        continue;
                    }
                    let sel_id = format!("SELECT_{}", opt_idx + 1);
                    let (opt_text, label) = if let Some(colon) = opt_trimmed.rfind(':') {
                        (&opt_trimmed[..colon], &opt_trimmed[colon+1..])
                    } else {
                        (opt_trimmed, "")
                    };

                    if let Some(ru_opt) = translations.get(&sel_id) {
                        let ruf_opt = apply_ru_f(ru_opt.trim());
                        new_parts.push(format!("{}:{}", ruf_opt, label));
                        file_imported_count += 1;
                    } else {
                        new_parts.push(opt.to_string());
                    }
                }
                new_lines.push(new_parts.join("\t"));
            } else {
                new_lines.push(line.clone());
            }
        }

        let out_script_path = out_scr_dir.join(script_name);
        if let Some(p) = out_script_path.parent() {
            fs::create_dir_all(p).unwrap();
        }

        // Encode as Shift-JIS / CP932 with CRLF line endings
        let content_to_write = new_lines.join("\r\n") + "\r\n";
        let (sjis_bytes, _, _) = encoding_rs::SHIFT_JIS.encode(&content_to_write);

        let mut out_f = File::create(&out_script_path).unwrap();
        out_f.write_all(&sjis_bytes).unwrap();

        total_imported_lines += file_imported_count;
        total_files += 1;
        println!("  [+] Импортирован: {} (вставлено {} переводов) -> {:?}", script_name, file_imported_count, out_script_path.file_name().unwrap());
    }

    println!("\n[+] Импорт завершен! Обработано файлов: {}, всего вставлено переводов: {}", total_files, total_imported_lines);
}

fn main() {
    let s_init = load_s_boxes();
    let keys = load_keys_from_exe();

    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        println!("Musica PAZ unpacker/packer & localization tool");
        println!("Использование:");
        println!("  Распаковка:  wind_paz.exe <файл.paz>");
        println!("  Запаковка:   wind_paz.exe pack <папка_с_файлами> <выходной.paz>");
        println!("  Экспорт MD:  wind_paz.exe export-md [en_dir] [jp1_dir] [jp_dir] [out_md_dir]");
        println!("  Импорт MD:   wind_paz.exe import-md [md_dir] [template_dir] [out_scr_dir] [wrap_limit]");
        return;
    }

    if args[1] == "pack" {
        if args.len() < 4 {
            println!("Ошибка: укажите папку и выходной файл!");
            println!("Пример: wind_paz.exe pack r:\\Wind\\unpacked_scr2 r:\\Wind\\scr2.paz");
            return;
        }
        pack_paz(Path::new(&args[2]), Path::new(&args[3]), &s_init, &keys);
    } else if args[1] == "export-md" {
        let en_dir = if args.len() > 2 { PathBuf::from(&args[2]) } else { PathBuf::from("r:\\Wind\\unpacked_scr2") };
        let jp1_dir = if args.len() > 3 { PathBuf::from(&args[3]) } else { PathBuf::from("r:\\Wind\\unpacked_scr1") };
        let jp0_dir = if args.len() > 4 { PathBuf::from(&args[4]) } else { PathBuf::from("r:\\Wind\\unpacked_scr") };
        let out_dir = if args.len() > 5 { PathBuf::from(&args[5]) } else { PathBuf::from("r:\\Wind\\scripts_md") };
        export_md(&en_dir, &jp1_dir, &jp0_dir, &out_dir);
    } else if args[1] == "import-md" {
        let md_dir = if args.len() > 2 { PathBuf::from(&args[2]) } else { PathBuf::from("r:\\Wind\\scripts_md") };
        let template_dir = if args.len() > 3 { PathBuf::from(&args[3]) } else { PathBuf::from("r:\\Wind\\unpacked_scr2") };
        let out_scr_dir = if args.len() > 4 { PathBuf::from(&args[4]) } else { PathBuf::from("r:\\Wind\\unpacked_scr2") };
        let wrap_limit = if args.len() > 5 { args[5].parse::<usize>().unwrap_or(56) } else { 56 };
        import_md(&md_dir, &template_dir, &out_scr_dir, wrap_limit);
    } else {
        unpack_paz(Path::new(&args[1]), &s_init, &keys);
    }
}
