use std::{
    io::{Read, Seek, SeekFrom},
    path::PathBuf,
};

use byteorder::{LittleEndian, ReadBytesExt};

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
struct FileTypeEntry {
    kind: u32,
    addr: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct FileNameEntry {
    index: u32,   // incrementing from zero
    name: String, // 64 bytes, presumably ascii
    unk2: u32,    // seemingly always 0 for wav, always 9 for mp3?
    size_in_bytes: u32,
    sample_rate: u32,            // seemingly always 44100 for wav, always 0 for mp3?
    unk5: u32,                   // seemingly always 8 or 16 for wav, always 0 for mp3?
    probably_channel_count: u32, // seemingly always 1 or 2 for wav, always 0 for mp3?
}

fn read_bytes_arr<const N: usize, R: Read>(r: &mut R) -> Result<[u8; N], std::io::Error> {
    let mut arr = [0; N];
    r.read_exact(&mut arr)?;
    Ok(arr)
}

fn read_bytes_vec<R: Read>(r: &mut R, len: usize) -> Result<Vec<u8>, std::io::Error> {
    let mut vec = vec![0; len];
    r.read_exact(&mut vec)?;
    Ok(vec)
}

// probably a better way to do this
fn read_padded_string<const N: usize, R: Read>(r: &mut R) -> Result<String, std::io::Error> {
    let bytes = read_bytes_arr::<N, R>(r)?;
    let mut s = String::new();
    for b in bytes {
        if b != 0 {
            s.push(b as char);
        } else {
            break;
        }
    }
    Ok(s)
}

fn read_file_type_entry<R: Read>(r: &mut R) -> Result<FileTypeEntry, std::io::Error> {
    Ok(FileTypeEntry {
        kind: r.read_u32::<LittleEndian>()?,
        addr: r.read_u32::<LittleEndian>()?,
    })
}

fn read_file_type_table<R: Read>(
    r: &mut R,
    num_files: u32,
) -> Result<Vec<FileTypeEntry>, std::io::Error> {
    (0..num_files)
        .map(|_| read_file_type_entry(r))
        .collect::<Result<Vec<_>, std::io::Error>>()
}

fn read_file_name_entry<R: Read>(r: &mut R) -> Result<FileNameEntry, std::io::Error> {
    Ok(FileNameEntry {
        index: r.read_u32::<LittleEndian>()?,
        name: read_padded_string::<64, R>(r)?,
        unk2: r.read_u32::<LittleEndian>()?,
        size_in_bytes: r.read_u32::<LittleEndian>()?,
        sample_rate: r.read_u32::<LittleEndian>()?,
        unk5: r.read_u32::<LittleEndian>()?,
        probably_channel_count: r.read_u32::<LittleEndian>()?,
    })
}

fn read_file_name_table<R: Read>(
    r: &mut R,
    num_files: u32,
) -> Result<Vec<FileNameEntry>, std::io::Error> {
    (0..num_files)
        .map(|_| read_file_name_entry(r))
        .collect::<Result<Vec<_>, std::io::Error>>()
}

fn main() -> Result<(), std::io::Error> {
    let mut fh = std::fs::File::open("Game.rgs").unwrap();

    fh.seek(SeekFrom::End(0))?;
    let file_size = fh.stream_position()?;
    fh.seek(SeekFrom::Start(0))?;

    println!("file_size: {file_size:08x}");

    let magic = fh.read_u32::<LittleEndian>()?;
    assert!(magic == 0x52455334); // 'RES4'

    let filetypes_start = fh.read_u32::<LittleEndian>()?;
    let filetypes_end = fh.read_u32::<LittleEndian>()?;
    let filetypes_size = filetypes_end - filetypes_start;
    let filenames_size = fh.read_u32::<LittleEndian>()?;

    println!("filetypes_start: {filetypes_start:08x}");
    println!("filetypes_end: {filetypes_end:08x}");
    println!("filetypes_size: {filetypes_size:08x}");
    println!("filenames_size: {filenames_size:08x}");

    assert_eq!(file_size, (filetypes_end + filenames_size) as u64);

    fh.seek(SeekFrom::Start(filetypes_start as u64))?;

    let num_files = fh.read_u32::<LittleEndian>()?;

    assert_eq!(filenames_size, num_files * 88);

    let file_type_table = read_file_type_table(&mut fh, num_files)?;
    for entry in &file_type_table {
        assert!((entry.addr as u64) < file_size);
        assert!(entry.kind == 0x534E4432); // 'SND2'
    }

    assert_eq!(fh.stream_position()?, filetypes_end as u64);

    // each file has a name entry prepended to its data, but there's this big table at the end of the file too
    let unneccessary_file_name_table = read_file_name_table(&mut fh, num_files)?;

    // tests to learn my way around the format
    /*for index in 0..num_files {
        let filetype = &file_type_table[index as usize];
        let filename = &file_name_table[index as usize];

        assert_eq!(filename.index, index);

        match filename.unk2 {
            0 => assert!(filename.name.ends_with(".wav")),
            9 => assert!(filename.name.ends_with(".mp3")),
            unk2 => eprintln!("[{}] unknown unk2 {}", filename.name, unk2),
        }

        match filename.unk5 {
            8 | 16 => assert!(filename.name.ends_with(".wav")),
            0 => assert!(filename.name.ends_with(".mp3")),
            unk5 => eprintln!("[{}] unknown unk5 {}", filename.name, unk5),
        }

        match filename.probably_channel_count {
            1 | 2 => assert!(filename.name.ends_with(".wav")),
            0 => assert!(filename.name.ends_with(".mp3")),
            probably_channel_count => eprintln!(
                "[{}] unknown probably_channel_count {}",
                filename.name, probably_channel_count
            ),
        }

        println!(
            "@{} {}: {}",
            filetype.addr, filename.name, filename.probably_channel_count
        );
    }*/

    let output_dir = PathBuf::from("dump/");
    if !output_dir.exists() {
        std::fs::create_dir(&output_dir)?;
    }

    for index in 0..num_files {
        let filetype_entry = &file_type_table[index as usize];

        fh.seek(SeekFrom::Start(filetype_entry.addr as u64))?;

        let filename_entry = read_file_name_entry(&mut fh)?;

        println!("[{index}/{num_files}] extracting {}", filename_entry.name);

        if index < num_files - 1
            && fh.stream_position()? == file_type_table[(index + 1) as usize].addr as u64
        {
            // uh oh, the file is zero bytes?
            eprintln!(
                "{}: error extracting, seems this file is actually zero bytes?",
                filename_entry.name
            );
            continue;
        }

        assert_eq!(filename_entry, unneccessary_file_name_table[index as usize]);

        let bytes = read_bytes_vec(&mut fh, filename_entry.size_in_bytes as usize)?;

        if filename_entry.name.ends_with(".wav") && bytes[0..4] != *"RIFF".as_bytes() {
            eprintln!("{}: wav file has bad RIFF header?", filename_entry.name);
        }

        std::fs::write(output_dir.join(&filename_entry.name), bytes)?;
    }

    Ok(())
}
