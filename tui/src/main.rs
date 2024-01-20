use shell_iface::Shell;

fn main() {
    let mut shell = Shell::new();

   installer::install(&shell);
}
