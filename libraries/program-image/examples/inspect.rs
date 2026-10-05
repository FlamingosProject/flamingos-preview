//! Inspect an executable using the kernel's image preparation code on the host.

fn main() {
    let path = std::env::args().nth(1).expect("usage: inspect <elf-file>");
    let bytes = std::fs::read(path).expect("could not read ELF file");
    let image = program_image::ProgramImage::from_elf(&bytes).expect("could not load ELF image");
    println!("entry {:#x}", image.entry);
    for segment in image.segments {
        println!(
            "address {:#x}, {} bytes, permissions {:?}",
            segment.virtual_address,
            segment.data.len(),
            segment.permissions
        );
    }
}
