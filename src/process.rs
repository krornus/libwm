use fork::Fork;

pub fn spawn<T>(args: &[T])
    where T: AsRef<str>
{
    if let Ok(Fork::Child) = fork::fork() {
        fork::setsid().expect("setsid failed");

        /* swap to const pointers. into_raw() can leak here
         * because we will execvp() or unreachable!() */
        let mut cs: Vec<_> = args
            .iter()
            .map(|x| {
                std::ffi::CString::new(x.as_ref())
                    .expect("spawn: invalid arguments")
                    .into_raw()
            })
            .collect();

        /* null ptr terminate the list */
        cs.push(std::ptr::null_mut());

        /* double fork so init handles zombies */
        if let Ok(Fork::Child) = fork::fork() {
            unsafe {
                libc::execvp(cs[0], (&cs[..]).as_ptr() as *const *const i8);
            }

            eprintln!("failed to spawn process");
            std::process::exit(1);
        }
    }
}
