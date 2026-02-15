fn reconcile(ctx) {
  let out = #{};

  if ctx.runtime_args.contains("--all-ns") {
    out["append_args"] = ["-A"];
  }

  if ctx.runtime_args.contains("--prod") {
    out["set_env"] = #{ "APP_ENV": "production" };
  }

  out
}
