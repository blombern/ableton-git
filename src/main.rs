mod parse;

use anyhow::{anyhow, Result};
use clap::{App, Arg};
use flate2::read::GzDecoder;
use git2::{IndexAddOption, Repository};
use parse::{parse_project, AbletonProject};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
};
use walkdir::WalkDir;

const SAMPLE_DIR: &str = "Samples";
const VC_SAMPLE_DIR: &str = "GitSamples";

fn main() {
    let matches = App::new("Ableton project version control")
        .version("0.1")
        .arg(
            Arg::new("input")
                .short('i')
                .long("input")
                .value_name("FILE")
                .required(true)
                .takes_value(true),
        )
        .get_matches();

    let input_file = matches.value_of("input").unwrap();

    let project = read_ableton_project(input_file).expect("Error reading Ableton project file");
    find_and_copy_samples(&project).expect("Error copying samples");
    git_commit(&project.project_dir).expect("Error committing files to Git");
}

fn read_ableton_project(path_str: &str) -> Result<AbletonProject> {
    let file_path = Path::new(path_str);
    let project_dir = file_path
        .parent()
        .ok_or(anyhow!("Unable to get parent directory of project file"))?;
    let project_file = fs::File::open(file_path)?;
    let mut gz = GzDecoder::new(project_file);
    let mut xml_content = String::new();
    gz.read_to_string(&mut xml_content)?;

    let project = parse_project(project_dir, &xml_content[..])?;

    println!(
        "Successfully parsed project, found {} samples",
        &project.samples.len()
    );

    println!("{:?}", &project);

    Ok(project)
}

fn find_and_copy_samples(project: &AbletonProject) -> Result<()> {
    let output_dir = &project.project_dir.join(VC_SAMPLE_DIR);
    if !output_dir.exists() {
        fs::create_dir_all(output_dir)?;
    }

    for sample in project.samples.iter() {
        let source_path = &project.project_dir.join(sample.as_path());
        println!("Looking for sample at {:?}", source_path);
        if source_path.exists() {
            let dest_path =
                output_dir.join(source_path.strip_prefix(&project.project_dir.join(SAMPLE_DIR))?);
            println!("Copying {:?} to {:?}", source_path, dest_path);
            let dest_parent = dest_path.parent().ok_or(anyhow!(
                "Unable to get parent directory of destination path: {:?}",
                &dest_path
            ))?;
            fs::create_dir_all(dest_parent)?;
            fs::copy(source_path, dest_path)?;
        }
    }
    Ok(())
}

fn git_commit(output_dir: &PathBuf) -> Result<()> {
    let dir = output_dir.to_str().expect("Unable to convert path to str");
    let repo = Repository::init(dir)?;
    let mut index = repo.index()?;
    let tree_id = {
        let mut options = git2::IndexAddOption::empty();
        options.insert(IndexAddOption::DEFAULT);
        for entry in WalkDir::new(output_dir) {
            let entry = entry?;
            let path = entry.path();
            index.add_path(path.strip_prefix(output_dir)?)?;
        }
        index.write_tree()?
    };

    let tree = repo.find_tree(tree_id)?;
    let head = repo.head().ok();
    let head_commit = head.as_ref().and_then(|h| h.peel_to_commit().ok());

    let signature = repo.signature()?;
    let message = "Add samples from Ableton project";
    let _ = repo.commit(
        Some("HEAD"),
        &signature,
        &signature,
        &message,
        &tree,
        head_commit.iter().collect::<Vec<_>>().as_slice(),
    )?;

    // Setup Git LFS
    let attrs_content = "* filter=lfs diff=lfs merge=lfs -text";
    fs::write(Path::new(output_dir).join(".gitattributes"), attrs_content)?;

    // Add .gitattributes file to index
    index.add_path(Path::new(".gitattributes"))?;
    index.write()?;
    let tree_id = index.write_tree()?;
    let tree = repo.find_tree(tree_id)?;

    // Commit .gitattributes
    let _ = repo.commit(
        Some("HEAD"),
        &signature,
        &signature,
        "Add .gitattributes for Git LFS",
        &tree,
        head_commit.iter().collect::<Vec<_>>().as_slice(),
    )?;

    Ok(())
}
