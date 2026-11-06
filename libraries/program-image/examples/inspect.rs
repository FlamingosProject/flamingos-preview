//! Inspect an executable using the kernel's image preparation code on the host.

fn main() {
    let path = std::env::args().nth(1).expect("usage: inspect <elf-file>");
    let bytes = std::fs::read(path).expect("could not read ELF file");
    let image = program_image::ProgramImage::from_elf(&bytes).expect("could not load ELF image");
    println!("entry {:#x}", image.entry);
    for segment in &image.segments {
        println!(
            "address {:#x}, {} bytes, permissions {:?}",
            segment.virtual_address,
            segment.data.len(),
            segment.permissions
        );
    }
    let memory = program_image::pages::PreparedImage::new(image, 1 << 30)
        .expect("could not prepare backing for the lower 1 GiB process address space");
    let pages: usize = memory
        .segments()
        .iter()
        .map(|segment| segment.pages().len())
        .sum();
    println!("prepared {pages} aligned pages (mappings not active)");
}
