use crate::format::format_bytes;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

/// Images smaller than this are not worth a notice.
pub const MIN_NOTICE_BYTES: u64 = 1024 * 1024;

pub const DOCKER_REMEDY: &str = "To return space to macOS: Docker Desktop > Settings > Resources > Disk image size (reduce it), or Troubleshoot > Clean / Purge data (deletes ALL images, containers and volumes).";

pub const COLIMA_REMEDY: &str = "To return space to macOS: run `colima ssh -- sudo fstrim -av` to ask the VM to discard unused blocks; if the file still does not shrink, `colima delete` recreates the VM (it removes ALL containers, images and volumes).";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiskImage {
    pub path: PathBuf,
    /// Bytes actually allocated on disk (st_blocks * 512).
    pub physical: u64,
    /// Apparent size, i.e. the maximum the sparse file can grow to.
    pub logical: u64,
}

pub fn probe(path: &Path) -> Option<DiskImage> {
    let meta = std::fs::metadata(path).ok()?;
    if !meta.is_file() {
        return None;
    }
    Some(DiskImage {
        path: path.to_path_buf(),
        physical: meta.blocks() * 512,
        logical: meta.len(),
    })
}

#[derive(Debug, Clone)]
pub struct DiskImageNotice {
    label: &'static str,
    candidates: Vec<PathBuf>,
    remedy: &'static str,
}

impl DiskImageNotice {
    pub fn new(label: &'static str, candidates: Vec<PathBuf>, remedy: &'static str) -> Self {
        Self {
            label,
            candidates,
            remedy,
        }
    }

    pub fn find(&self) -> Vec<DiskImage> {
        self.candidates
            .iter()
            .filter_map(|p| probe(p))
            .filter(|i| i.physical >= MIN_NOTICE_BYTES)
            .collect()
    }

    pub fn physical_total(&self) -> u64 {
        self.find().iter().map(|i| i.physical).sum()
    }

    pub fn render(&self) -> Option<String> {
        let images = self.find();
        if images.is_empty() {
            return None;
        }
        let mut out = format!(
            "[{}] virtual disk image (pruning inside the VM does not always shrink it):\n",
            self.label
        );
        for i in &images {
            out.push_str(&format!(
                "  {}\n    on disk: {} / maximum size: {}\n",
                i.path.display(),
                format_bytes(i.physical),
                format_bytes(i.logical)
            ));
        }
        out.push_str(&format!("  {}", self.remedy));
        Some(out)
    }

    pub fn after_clean_message(&self, before: u64, after: u64) -> Option<String> {
        if before == 0 {
            return None;
        }
        // Shrinking by 2% or less counts as "did not shrink" to absorb block rounding.
        if after + before / 50 < before {
            Some(format!(
                "[{}] virtual disk on disk: {} -> {}",
                self.label,
                format_bytes(before),
                format_bytes(after)
            ))
        } else {
            Some(format!(
                "[{}] virtual disk did not shrink: {} -> {}\n  {}",
                self.label,
                format_bytes(before),
                format_bytes(after),
                self.remedy
            ))
        }
    }
}

pub fn docker_desktop_candidates(home: &Path) -> Vec<PathBuf> {
    let data = home.join("Library/Containers/com.docker.docker/Data");
    vec![
        data.join("vms/0/data/Docker.raw"),
        data.join("vms/0/Docker.raw"),
        data.join("com.docker.driver.amd64-linux/Docker.qcow2"),
        data.join("com.docker.driver.amd64-linux/Docker.raw"),
    ]
}

pub fn colima_candidates(home: &Path) -> Vec<PathBuf> {
    let lima = home.join(".colima/_lima");
    let mut out = vec![];
    for dir in subdirs(&lima) {
        if dir.file_name().is_some_and(|n| n == "_disks") {
            for disk in subdirs(&dir) {
                out.push(disk.join("datadisk"));
            }
        } else {
            out.push(dir.join("disk"));
            out.push(dir.join("diffdisk"));
        }
    }
    out.sort();
    out
}

fn subdirs(dir: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(dir)
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .map(|e| e.path())
                .filter(|p| p.is_dir())
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use std::io::Write;
    use tempfile::TempDir;

    const MIB: usize = 1024 * 1024;

    /// Writes `real` bytes of data, then extends the file to `logical` bytes (sparse tail).
    fn make_image(path: &Path, real: usize, logical: u64) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        let mut f = File::create(path).unwrap();
        f.write_all(&vec![1u8; real]).unwrap();
        f.set_len(logical).unwrap();
    }

    fn notice_for(path: PathBuf) -> DiskImageNotice {
        DiskImageNotice::new("docker", vec![path], "REMEDY TEXT")
    }

    #[test]
    fn probe_reports_physical_below_logical_for_sparse_file() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("Docker.raw");
        make_image(&p, 2 * MIB, 64 * MIB as u64);
        let img = probe(&p).unwrap();
        assert_eq!(img.logical, 64 * MIB as u64);
        assert!(img.physical >= 2 * MIB as u64);
        assert!(img.physical < img.logical);
    }

    #[test]
    fn probe_returns_none_for_missing_or_directory() {
        let tmp = TempDir::new().unwrap();
        assert!(probe(&tmp.path().join("nope")).is_none());
        assert!(probe(tmp.path()).is_none());
    }

    #[test]
    fn find_ignores_images_below_threshold() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("tiny.raw");
        make_image(&p, 10, 10);
        assert!(notice_for(p).find().is_empty());
    }

    #[test]
    fn render_is_none_without_images() {
        let tmp = TempDir::new().unwrap();
        assert!(notice_for(tmp.path().join("nope")).render().is_none());
    }

    #[test]
    fn render_lists_path_sizes_and_remedy() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("Docker.raw");
        make_image(&p, 2 * MIB, 64 * MIB as u64);
        let text = notice_for(p).render().unwrap();
        assert!(text.contains("[docker]"), "{text}");
        assert!(text.contains("Docker.raw"), "{text}");
        assert!(text.contains("on disk:"), "{text}");
        assert!(text.contains("64.0 MB"), "{text}");
        assert!(text.contains("REMEDY TEXT"), "{text}");
    }

    #[test]
    fn after_clean_message_reports_shrink() {
        let n = notice_for(PathBuf::from("/nonexistent"));
        let msg = n
            .after_clean_message(100 * MIB as u64, 50 * MIB as u64)
            .unwrap();
        assert!(msg.contains("100.0 MB -> 50.0 MB"), "{msg}");
        assert!(!msg.contains("did not shrink"), "{msg}");
    }

    #[test]
    fn after_clean_message_warns_when_not_shrunk() {
        let n = notice_for(PathBuf::from("/nonexistent"));
        let msg = n
            .after_clean_message(100 * MIB as u64, 99 * MIB as u64)
            .unwrap();
        assert!(msg.contains("did not shrink"), "{msg}");
        assert!(msg.contains("REMEDY TEXT"), "{msg}");
    }

    #[test]
    fn after_clean_message_is_none_when_nothing_was_measured() {
        let n = notice_for(PathBuf::from("/nonexistent"));
        assert!(n.after_clean_message(0, 0).is_none());
    }

    #[test]
    fn docker_candidates_start_with_current_layout() {
        let c = docker_desktop_candidates(Path::new("/h"));
        assert_eq!(
            c[0],
            PathBuf::from("/h/Library/Containers/com.docker.docker/Data/vms/0/data/Docker.raw")
        );
        assert_eq!(c.len(), 4);
    }

    #[test]
    fn colima_candidates_find_disk_and_datadisk() {
        let tmp = TempDir::new().unwrap();
        let lima = tmp.path().join(".colima/_lima");
        fs::create_dir_all(lima.join("colima")).unwrap();
        fs::create_dir_all(lima.join("_disks/colima")).unwrap();
        let c = colima_candidates(tmp.path());
        assert!(c.contains(&lima.join("colima/disk")), "{c:?}");
        assert!(c.contains(&lima.join("_disks/colima/datadisk")), "{c:?}");
    }

    #[test]
    fn colima_candidates_empty_without_colima_dir() {
        let tmp = TempDir::new().unwrap();
        assert!(colima_candidates(tmp.path()).is_empty());
    }
}
