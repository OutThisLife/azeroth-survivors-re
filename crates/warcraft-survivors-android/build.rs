//! Stamps the commit this library was built from (runs on the host, also when cross-compiling).

fn main() {
    benilla_buildstamp::emit();
}
