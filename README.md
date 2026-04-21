Projet RUST SLUB ALLOCATOR - SLIMANE Nadir & BERBER Aissa - 4SIJ



To run exectution :

```cargo bootimage -Z json-target-specµ```

```cargo xtest --test heap_allocator```


To see the test & result :
```cargo test -Z json-target-spec```

```qemu-system-x86_64   -drive format=raw,file=target/x86_64-rust_esgi/debug/bootimage-rust_esgi.bin   -serial mon:stdio```

