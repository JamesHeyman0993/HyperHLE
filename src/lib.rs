    if let Some(version) = minimum_os_version {
        let (major, minor_etc) = version.split_once('.').unwrap();
        let minor = minor_etc
            .split_once('.')
            .map_or(minor_etc, |(minor, _etc)| minor);
        let major: u32 = major.parse().unwrap();
        let minor: u32 = minor.parse().unwrap();
        if major > 6 || (major == 6 && minor > 0) {
            echo!("Warning: app requires OS version {}. Only apps for iOS 6.0 and earlier are currently supported.", version);
        }
    }
