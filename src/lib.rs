use std::{
    fs::{self, DirEntry},
    io,
    path::Path,
    sync::{Arc, Mutex},
    thread, vec,
};

#[derive(Default, Debug)]
pub struct Song {
    name: String,
    dominant_dir: String,
    extension_name: String,
    absolute_path: String,
}

pub fn get_user_list(list: &str) -> Vec<String> {
    match fs::read_to_string(list) {
        Ok(data) => data
            .split("\n")
            .map(|s| s.to_string())
            .collect::<Vec<String>>(),
        Err(error) => panic!("fail to open music list: {error}"),
    }
}

pub fn get_local_song(dir: &str) -> Vec<Song> {
    let mut local_song = vec![];
    let _ = visit_dirs(Path::new(dir), &mut |entry: &DirEntry| {
        let mut song = Song::default();

        if let Some(extension_name) = entry.path().extension() {
            song.extension_name = extension_name.to_os_string().to_str().unwrap().to_string();
        }

        song.name = entry
            .path()
            .file_stem()
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();

        song.dominant_dir = entry.path().parent().unwrap().to_str().unwrap().to_string();
        song.absolute_path = fs::canonicalize(entry.path())
            .unwrap()
            .to_str()
            .unwrap()
            .to_string();

        local_song.push(song);
    });

    local_song
}

pub fn list_to_m3u_fuzzy_parallel(
    user_list: &[String],
    local_song: &[Song],
    is_absolute_path: bool,
) -> (Vec<String>, Vec<String>) {
    let local_song = Arc::new(local_song);

    let cpu_cores = thread::available_parallelism()
        .expect("Failed to get thread count")
        .get();

    let mut result = vec![Some(String::new()); user_list.len()];

    let miss_match = Arc::new(Mutex::new(vec![]));

    let chunk_size = user_list.len().div_ceil(cpu_cores);

    thread::scope(|s| {
        let result_chunks = result.chunks_mut(chunk_size);

        for (chunk_idx, (user_chunk, res_chunk)) in
            user_list.chunks(chunk_size).zip(result_chunks).enumerate()
        {
            let chunk = user_chunk.to_vec();
            let local_song_p = Arc::clone(&local_song);
            let miss_match_p = Arc::clone(&miss_match);
            let start_idx = chunk_idx * chunk_size;

            let parallel_clo = move || {
                for (chunk_offset, user_list_song) in chunk.iter().enumerate() {
                    let user_list_index = start_idx + chunk_offset;

                    match find_best_match(user_list_song, &local_song_p) {
                        Some((match_song, match_rate)) => {
                            if match_rate < 0.6 {
                                let mut miss_match_lock = miss_match_p.lock().unwrap();
                                miss_match_lock.push((
                                    user_list_index,
                                    format!(
                                        "target: {}.{}\nmatch_song:{}\nmatch_rate: {:.2}\n ",
                                        user_list_index,
                                        &user_list_song,
                                        match_song.name,
                                        match_rate
                                    ),
                                ));
                                res_chunk[chunk_offset] = None;
                                continue;
                            }

                            let full_output = {
                                if is_absolute_path {
                                    Some(match_song.absolute_path.to_string())
                                } else {
                                    Some(format!(
                                        "{}/{}.{}",
                                        match_song.dominant_dir,
                                        match_song.name,
                                        match_song.extension_name
                                    ))
                                }
                            };

                            res_chunk[chunk_offset] = full_output;
                        }
                        None => panic!("Fail to match"),
                    }
                }
            };

            s.spawn(parallel_clo);
        }
    });

    miss_match
        .lock()
        .unwrap()
        .sort_by(|(a, _), (b, _)| a.cmp(b));

    let mut hit_match = vec![];
    let mut miss_match_clone = vec![];

    println!("#EXTM3U");
    for msg in result.into_iter().flatten() {
        println!("{msg}");
        hit_match.push(msg);
    }

    for (_, msg) in miss_match.lock().unwrap().iter() {
        eprintln!("{msg}");
        miss_match_clone.push(msg.to_string());
    }

    (hit_match, miss_match_clone)
}

pub fn list_to_m3u_fuzzy(
    user_list: &[String],
    local_song: &[Song],
    is_absolute_path: bool,
) -> (Vec<String>, Vec<String>) {
    let mut miss_match = vec![];
    let mut hit_match = vec![];

    println!("#EXTM3U");
    for (index, song) in user_list.iter().enumerate() {
        match find_best_match(song, local_song) {
            Some((match_song, match_rate)) => {
                if match_rate < 0.6 {
                    miss_match.push(format!(
                        "target: {}.{}\nmatch_song:{}\nmatch_rate: {:.2}\n ",
                        index, song, match_song.name, match_rate
                    ));
                    continue;
                }

                let full_output = {
                    if is_absolute_path {
                        match_song.absolute_path.to_string()
                    } else {
                        format!(
                            "{}/{}.{}",
                            match_song.dominant_dir, match_song.name, match_song.extension_name
                        )
                    }
                };
                println!("{}", full_output);
                hit_match.push(full_output);
            }
            None => panic!("Fail to match"),
        }
    }

    for miss in miss_match.iter() {
        eprintln!("{}", miss);
    }

    (hit_match, miss_match)
}

fn find_best_match<'a>(target: &str, candidates: &'a [Song]) -> Option<(&'a Song, f64)> {
    candidates
        .iter()
        .map(|cand| (cand, strsim::normalized_levenshtein(target, &cand.name)))
        .max_by(|(_, sim1), (_, sim2)| sim1.partial_cmp(sim2).unwrap())
}

fn visit_dirs(dir: &Path, cb: &mut dyn FnMut(&DirEntry)) -> io::Result<()> {
    if dir.is_dir() {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                visit_dirs(&path, cb)?;
            } else {
                cb(&entry);
            }
        }
    }
    Ok(())
}
