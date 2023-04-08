use anyhow::Result;
use quick_xml::{events::Event, name::QName, Reader};
use std::path::{Path, PathBuf};
use std::str::from_utf8;

#[derive(Debug)]
pub struct AbletonProject {
    pub project_dir: PathBuf,
    pub samples: Vec<PathBuf>,
}

pub fn parse_project(project_dir: &Path, xml: &str) -> Result<AbletonProject> {
    let mut reader = Reader::from_reader(xml.as_bytes());
    let mut samples = Vec::new();

    let mut in_sample_ref = false;
    let mut in_file_ref = false;

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e)) if e.name() == QName(b"SampleRef") => {
                in_sample_ref = true;
            }
            Ok(Event::End(ref e)) if e.name() == QName(b"SampleRef") => {
                in_sample_ref = false;
            }
            Ok(Event::Start(ref e)) if e.name() == QName(b"FileRef") => {
                in_file_ref = true;
            }
            Ok(Event::End(ref e)) if e.name() == QName(b"FileRef") => {
                in_file_ref = false;
            }
            Ok(Event::Empty(ref e))
                if in_sample_ref && in_file_ref && e.name() == QName(b"RelativePath") =>
            {
                if let Some(attr) = e.try_get_attribute("Value")? {
                    let relative_path = PathBuf::from(from_utf8(&attr.value)?);
                    if is_audio_file(&relative_path) {
                        samples.push(relative_path);
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(err) => panic!("Error at position {}: {:?}", reader.buffer_position(), err),
            _ => (),
        }
    }

    samples.sort();
    samples.dedup();

    Ok(AbletonProject {
        project_dir: project_dir.to_path_buf(),
        samples,
    })
}

fn is_audio_file(file: &PathBuf) -> bool {
    file.as_path().extension().map_or(false, |ext| {
        ext == "wav" || ext == "aif" || ext == "aiff" || ext == "mp3" || ext == "flac"
    })
}
