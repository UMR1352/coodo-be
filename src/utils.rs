#[macro_export]
macro_rules! redis_fcall {
    ($f_name:ident) => {
        {
            let mut cmd = redis::cmd("FCALL");
            cmd
                .arg(stringify!($f_name))
                .arg(0);
            cmd
        }
    };
    ($f_name:ident, $key1:expr, $key2:expr, $($arg:expr),*) => {
        {
            let mut cmd = redis::cmd("FCALL");
            cmd
                .arg(stringify!($f_name))
                .arg(2)
                .arg($key1)
                .arg($key2)
                $(.arg($arg))*;
            cmd
        }
    };
    ($f_name:ident, $key:expr $(, $arg:expr),*) => {
        {
            let mut cmd = redis::cmd("FCALL");
            cmd
                .arg(stringify!($f_name))
                .arg(1)
                .arg($key)
                $(.arg($arg))*;
            cmd
        }
    };
}
