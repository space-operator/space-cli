const std = @import("std");

pub fn build(b: *std.build.Builder) void {
    const mode = b.standardReleaseOptions();
    const lib = b.addSharedLibrary("<%= name %>", "src/main.zig", .unversioned);
    lib.setTarget(.{ .cpu_arch = .wasm32, .os_tag = .wasi });
    lib.setBuildMode(mode);
    lib.install();
}
