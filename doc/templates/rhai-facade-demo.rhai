fn reconcile(ctx) {
  let out = #{};

  let p = platform_info();
  out["set_env"] = #{
    "CHOPPER_PLATFORM_OS": p["os"],
    "CHOPPER_PLATFORM_ARCH": p["arch"],
  };

  if fs_exists("runtime-args.txt") {
    let extra = fs_read_text("runtime-args.txt");
    out["append_args"] = [extra.trim()];
  }

  let probe = proc_run("sh", ["-c", "echo probe-ok"], 1000);
  if probe["ok"] {
    out["set_env"]["CHOPPER_PROBE"] = "ok";
  }

  out
}

