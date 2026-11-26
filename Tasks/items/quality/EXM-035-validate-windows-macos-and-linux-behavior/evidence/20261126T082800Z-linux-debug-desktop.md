# EXM-035 Linux debug desktop qualification

Runtime source: 58f5eb938998e6ee9d5dffd4f5f430be2c7da3b6, with EXM-101
product correction 9b53499; later 1ee4da7 changes only qualification records/docs.
Rust 1.98.0, committed Cargo.lock, Zig 0.15.2 GNU wrapper targeting glibc 2.35.
QEMU 11.1.0 q35 TCG, 4 vCPUs/4 GiB RAM, Ubuntu 24.04.5 x64 cloud image,
kernel 6.8.0-147-generic. Exact trusted image/tool hashes are in the preceding
035 evidence. All data/devices are synthetic; no physical passthrough.

## Build and environment

```text
cargo +1.98.0 build -p exmon-gui --target x86_64-unknown-linux-gnu --locked -j 2
cargo +1.98.0 build -p exmon-gui --examples --target x86_64-unknown-linux-gnu --locked -j 2
cargo +1.98.0 build -p exmon-gui --example native_hid_smoke --features native-hid --target x86_64-unknown-linux-gnu --locked -j 2
```

All three builds succeeded. The all-examples build reported an unused variable
at performance_soak.rs:273: its deadline is used only in the Windows removal
branch. This requires a separate correction before the strict cross-target
lint gate. No lint allowance or source change was made here.

The ordinary user exmon runs Xvfb :99, 1400x1000x24, with -nolisten tcp.
Product window size is 1200x760. Environment requests WGPU_BACKEND=vulkan,
LIBGL_ALWAYS_SOFTWARE=1 and a private mode-0700 XDG_RUNTIME_DIR. A bounded
Vulkan loader inventory found exactly one adapter: llvmpipe (LLVM 20.1.2,
128 bits), CPU type 4. Packages:

- libvulkan1 1.3.275.0-1build1
- mesa-vulkan-drivers 25.2.8-0ubuntu0.24.04.5
- xvfb 2:21.1.12-1ubuntu1.9
- libx11-6 2:1.8.7-1ubuntu0.1
- libxcursor1 1:1.2.1-1build1
- libxi6 2:1.8.1-1build1
- libxkbcommon-x11-0 1.6.0-1build1

Initial startup correctly returned an explicit missing libXcursor library error.
Installing the missing X11 dependencies resolved that startup failure. Xvfb
reported nonfatal unknown XF86 keysyms; its own log says they are not fatal.
The production window then rendered; initial software-renderer startup took
about a minute. This is not a desktop responsiveness/performance pass.

## Unsuccessful bounded screenshot tests

Each unchanged native example ran alone, -- no parallel GUI process -- under
its existing eight-second internal deadline and a 90-second external process
watchdog. All eight exited 1 with screenshot/view completion errors. No deadline
was changed and no failed example is counted as passed:

| Example | Stripped executed artifact SHA-256 |
| --- | --- |
| native_shell_smoke | 95d145ece4f59f8c3c8fc38e1b2a62580566c7ad8318896563a9f6da541b9a19 |
| native_incoming_smoke | 9d0386394636d412e955cde10f5aab9b1819b06e9cc99ae433987315682f15be |
| native_outgoing_smoke | 68d3feccd21f72e0caea53bebcb6fdaf0e33f7be4322c853dde2ac7f64e50ecb |
| native_charts_smoke | 25fcf4e45a59be40bfd00a5135b1d827722a3d9e7088b57f80196161912ddf3e |
| native_settings_smoke | 43ecf42b5d3bcd4f1a15f001367025fe884219dc2d79564f99116986238ad7e5 |
| native_inspection_smoke | 9a8775b09ee1fc7645a2823f1d94ce64c0b5ceb197dc1596e8a925f682ba3be6 |
| native_sessions_smoke | fad42bd08909eefe424260864807d7c56333e11608ee19818510c5fb33efedb2 |
| native_hid_smoke | 5808d4b9b2517b22cd773092f38dd06a8ccdd28d663f6ad38f3fb5124bc08c9c |

A standalone diagnostic copied native_shell_smoke, changed the viewport to
visible, redirected evidence paths, and added first-UI/close logging. It kept
the eight-second deadline and original success assertions. The first UI call
had 7529 ms remaining. At deadline closure it reported packets=61888, shot=true,
result=0, expired=true (then packets=61920). It also exited 1. Thus visibility
alone did not resolve completion; the worker made progress but the screenshot
did not finish by closure. TCG/debug rendering overhead is a hypothesis, pending
comparison with the optimized build. This diagnostic is not a canonical pass.

## Manual production-window observations

The production binary ran under the ordinary guest user. Explicit synthetic
Start, Stop, Charts and Settings clicks were sent to the displayed window.
Screenshots were copied and visually inspected at native resolution:

- gui-product.png: initial IDLE shell, mock selection, RX/TX panels and disabled TX.
- gui-manual-capture.png: CAPTURING, visible incoming rows, RX 16192 observations /
  518144 bytes, display decimation 15808, backend loss/application drops 0.
- gui-manual-stopped.png: IDLE after Stop, RX 60512 / 1936384 bytes, display
  decimation 59488, history evictions 512, backend loss/application drops 0.
- gui-manual-charts.png: chart controls/axes rendered, prepared field samples
  1536; no numeric series selected, so no plotted-series or average verdict.
- gui-manual-settings.png: still shows Charts while the next click is pending.
- gui-manual-theme.png: Settings eventually rendered; still dark, so theme
  switching is not verified by this screenshot. Settings edits were not attempted.

An initial teardown used xdotool windowclose, which forcibly destroys the X
window rather than sending the normal WM_DELETE_WINDOW request. Winit then
panicked on GetGeometry / X11 Drawable error 9. That forced-destruction scenario
is not a clean shutdown pass. The subsequent manual workflow sent a proper
WM_PROTOCOLS / WM_DELETE_WINDOW client message through a small Xlib helper,
waited for the owned product child, and emitted MANUAL_PRODUCT_CLEAN_EXIT 0.
Its product stderr log is empty. This establishes the observed normal-window
shutdown for this limited mock workflow, not the failed screenshot suite.

The VM was powered off with sudo poweroff; its owned QEMU child exited 0.
The synthetic overlay/toolcache is retained ignored for optimized comparison.
No guest credential, disk, executable, module or generated build output is
committed. Full Linux GUI, packaged install/permissions, Apple Silicon and
physical stand qualification remain pending. EXM-035 is not Done.

## Local evidence hashes

- `exmon-gui-linux`: `1195af7fd2e8a661021acc19dcb0060fd3d0ce9da7e719922ef37510a65a0ac3`
- `gui-build.log`: `167e0c2660d3e60e450006a8b38887139da78ebe82be109f6725b8c80e8ac9fe`
- `gui-examples-build.log`: `f4239c1fc5cf627f220a849bd47640cb2ef4323dd4c88c8ad2c67f3aa5dcb043`
- `gui-hid-build.log`: `531abac0bec7c5addf466a03135a82199e13900c85f171514a6a96983c6cc924`
- `gui-start-runtime.log`: `00e735b4bd266d3e7dbde96edbdf3bcbb9560ce316aff7c8c472e090cef5613f`
- `gui-relaunch-runtime.log`: `e0907949bd27233fef238b4d7ef7f5424c041980e2f4208c01f83db01470f892`
- `gui-smokes-runtime.log`: `a0d28a400c36cc900ab74116e3b5b1441cfebe27989837b2f6a19147d403cd1d`
- `graphics-inventory-runtime.log`: `a27935232f794740edf4a1f2a55ea7275f306de6a7c458e6079b26e9e67d8a09`
- `visible-qualification`: `4463b217a328cf45289c8ecf99177eb88172656f58807ac510b9b5034b956f14`
- `visible-build.log`: `85f7c487ac94fbeba12343e85eded37ae313e403d3572aa90c13b315fec96b19`
- `visible-runtime.log`: `969293b6a80b2684e8b2859c2766ed76aa7bdda51aeea6bbb67c2a5650762338`
- `guest-manual-workflow.sh`: `6285a5a5c331f0de56cf6cb815d1c2db1bed3a6ae92837eeff9670494c31d028`
- `wm-close.py`: `9030ccb347571c305b906c5c42560575bd78656dc0ac54cded3fdac1b8f8a72f`
- `manual-workflow-runtime.log`: `42207e19dce0df7f39c5f334d8188fb6ef28f65907e1544f4f1a49f3482ae561`
- `gui-manual-workflow.log`: `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855`
- `gui-product.png`: `573b235e7d65a04ddb4b3d95f78d42ca4742cf51afc59b200898fdad8f080458`
- `gui-product-missing-xcursor.log`: `0befd428c14707b6cc1bce98bdd487a615dcbd23050fafacc6f83051bbdbd2e2`
- `gui-manual-capture.png`: `c94ba7d4663868700cff1d8b1a01c32719f8562c787c7ab2c715981fbb7b2fc1`
- `gui-manual-stopped.png`: `fe63e98c3a7552d73470538b5c95f1da66a13bb145e08b163f7d6b172a2e23a1`
- `gui-manual-charts.png`: `c2dd4021a90affda93763a65ed5d34ee1d744368697962cbe318aca5de770eba`
- `gui-manual-settings.png`: `038467e88fff74b7891b2c8fd8b7429349be790a01018b2e5308a92366b8c05c`
- `gui-manual-theme.png`: `40829bcd0a8dbaaa4940f4661a7390d3377b3eb9aeffc1a1961b4b3546b3fad8`

Diagnostic source SHA-256: a636387bb1dc7ab9daad586596238d7e6d711c9a32357ff9df241736af651a5a.
