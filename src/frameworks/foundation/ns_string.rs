- (())encodeWithCoder:(id)coder {
    let class: Class = msg![env; coder class];
    let keyed_arch_class: Class = msg_class![env; NSKeyedArchiver class];
    if env.objc.class_is_subclass_of(class, keyed_arch_class) {
        let host = env.objc.borrow::<StringHostObject>(this);
        let rust_str = match &*host {
            StringHostObject::Utf8(s) => s.to_string(),
            StringHostObject::Utf16(s) => String::from_utf16_lossy(s).to_string(),
        };
        drop(host);
        
        // Create the key directly in guest memory to avoid creating another NSString object
        // that would need encoding, causing infinite recursion
        let key_str = "NS.string";
        let key = from_rust_string(env, key_str.to_string());
        
        // Encode the string content
        let content = from_rust_string(env, rust_str);
        () = msg![env; coder encodeObject:content forKey:key];
        
        // Immediately release to prevent reference cycles
        release(env, content);
        release(env, key);
    } else {
        println!("Warning: _touchHLE_NSString encodeWithCoder: unsupported coder class, skipping");
    }
}
