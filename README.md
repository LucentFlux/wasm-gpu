## Setup
Build and Install llvm libraries

```shell
cargo install llvmenv
llvmenv init
llvmenv build-entry 12.0.1
```

if this errors then try some of the following

https://github.com/llvmenv/llvmenv/issues/115

```shell
ln -s "$HOME/.cache/llvmenv/12.0.1/projects/libunwind/include/mach-o" "$HOME/.cache/llvmenv/12.0.1/tools/lld/MachO/mach-o"
```

```cmd
mklink /J "%LocalAppData%\llvmenv\12.0.1\tools\lld\MachO\mach-o" "%LocalAppData%\llvmenv\12.0.1\projects\libunwind\include\mach-o"
mklink /H "%LocalAppData%\llvmenv\12.0.1\build\lib\libomp.dll.lib" "%LocalAppData%\llvmenv\12.0.1\build\lib\Debug\libomp.dll.lib"
```

https://stackoverflow.com/questions/46108390/building-llvm-with-cmake-and-visual-stuidio-fails-to-install