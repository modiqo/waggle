var __defProp = Object.defineProperty;
var __name = (target, value) => __defProp(target, "name", { value, configurable: true });

// build/index.js
import { WorkerEntrypoint as xt } from "cloudflare:workers";
import Z from "./34fe31f20f39a35ca23b91836b53b9d3835f8512-index_bg.wasm";
var G = globalThis.__worker_init_state = { criticalError: false, instanceId: 0 };
var I = class {
  static {
    __name(this, "I");
  }
  __destroy_into_raw() {
    let t = this.__wbg_ptr;
    return this.__wbg_ptr = 0, bt.unregister(this), t;
  }
  free() {
    let t = this.__destroy_into_raw();
    c();
    try {
      i.__wbg_containerstartupoptions_free(t, 0);
    } catch (e) {
      s(e);
    }
  }
  get enableInternet() {
    if (this.__wbg_inst !== void 0 && this.__wbg_inst !== o) throw new Error("Invalid stale object from previous Wasm instance");
    let t;
    c();
    try {
      t = i.__wbg_get_containerstartupoptions_enableInternet(this.__wbg_ptr);
    } catch (e) {
      s(e);
    }
    return t === 16777215 ? void 0 : t !== 0;
  }
  get entrypoint() {
    if (this.__wbg_inst !== void 0 && this.__wbg_inst !== o) throw new Error("Invalid stale object from previous Wasm instance");
    let t;
    c();
    try {
      t = i.__wbg_get_containerstartupoptions_entrypoint(this.__wbg_ptr);
    } catch (n) {
      s(n);
    }
    var e = ht(t[0], t[1]).slice();
    return i.__wbindgen_free(t[0], t[1] * 4, 4), e;
  }
  get env() {
    if (this.__wbg_inst !== void 0 && this.__wbg_inst !== o) throw new Error("Invalid stale object from previous Wasm instance");
    let t;
    c();
    try {
      t = i.__wbg_get_containerstartupoptions_env(this.__wbg_ptr);
    } catch (e) {
      s(e);
    }
    return t;
  }
  set enableInternet(t) {
    if (this.__wbg_inst !== void 0 && this.__wbg_inst !== o) throw new Error("Invalid stale object from previous Wasm instance");
    c();
    try {
      i.__wbg_set_containerstartupoptions_enableInternet(this.__wbg_ptr, u(t) ? 16777215 : t ? 1 : 0);
    } catch (e) {
      s(e);
    }
  }
  set entrypoint(t) {
    if (this.__wbg_inst !== void 0 && this.__wbg_inst !== o) throw new Error("Invalid stale object from previous Wasm instance");
    let e = pt(t, i.__wbindgen_malloc), n = l;
    c();
    try {
      i.__wbg_set_containerstartupoptions_entrypoint(this.__wbg_ptr, e, n);
    } catch (_) {
      s(_);
    }
  }
  set env(t) {
    if (this.__wbg_inst !== void 0 && this.__wbg_inst !== o) throw new Error("Invalid stale object from previous Wasm instance");
    c();
    try {
      i.__wbg_set_containerstartupoptions_env(this.__wbg_ptr, t);
    } catch (e) {
      s(e);
    }
  }
};
Symbol.dispose && (I.prototype[Symbol.dispose] = I.prototype.free);
var E = class {
  static {
    __name(this, "E");
  }
  __destroy_into_raw() {
    let t = this.__wbg_ptr;
    return this.__wbg_ptr = 0, K.unregister(this), t;
  }
  free() {
    let t = this.__destroy_into_raw();
    c();
    try {
      i.__wbg_hive_free(t, 0);
    } catch (e) {
      s(e);
    }
  }
  alarm() {
    if (this.__wbg_inst !== void 0 && this.__wbg_inst !== o) throw new Error("Invalid stale object from previous Wasm instance");
    let t;
    c();
    try {
      t = i.hive_alarm(this.__wbg_ptr);
    } catch (e) {
      s(e);
    }
    return t;
  }
  fetch(t) {
    if (this.__wbg_inst !== void 0 && this.__wbg_inst !== o) throw new Error("Invalid stale object from previous Wasm instance");
    let e;
    c();
    try {
      e = i.hive_fetch(this.__wbg_ptr, t);
    } catch (n) {
      s(n);
    }
    return e;
  }
  constructor(t, e) {
    let n;
    c();
    try {
      n = i.hive_new(t, e);
    } catch (_) {
      s(_);
    }
    return this.__wbg_ptr = n, Object.defineProperty(this, "__wbg_inst", { value: o, writable: true }), K.register(this, { ptr: n, instance: o }, this), this;
  }
  webSocketClose(t, e, n, _) {
    if (this.__wbg_inst !== void 0 && this.__wbg_inst !== o) throw new Error("Invalid stale object from previous Wasm instance");
    let a = v(n, i.__wbindgen_malloc, i.__wbindgen_realloc), f = l, b;
    c();
    try {
      b = i.hive_webSocketClose(this.__wbg_ptr, t, e, a, f, _);
    } catch (g) {
      s(g);
    }
    return b;
  }
  webSocketError(t, e) {
    if (this.__wbg_inst !== void 0 && this.__wbg_inst !== o) throw new Error("Invalid stale object from previous Wasm instance");
    let n;
    c();
    try {
      n = i.hive_webSocketError(this.__wbg_ptr, t, e);
    } catch (_) {
      s(_);
    }
    return n;
  }
  webSocketMessage(t, e) {
    if (this.__wbg_inst !== void 0 && this.__wbg_inst !== o) throw new Error("Invalid stale object from previous Wasm instance");
    let n;
    c();
    try {
      n = i.hive_webSocketMessage(this.__wbg_ptr, t, e);
    } catch (_) {
      s(_);
    }
    return n;
  }
};
Symbol.dispose && (E.prototype[Symbol.dispose] = E.prototype.free);
var R = class {
  static {
    __name(this, "R");
  }
  __destroy_into_raw() {
    let t = this.__wbg_ptr;
    return this.__wbg_ptr = 0, wt.unregister(this), t;
  }
  free() {
    let t = this.__destroy_into_raw();
    c();
    try {
      i.__wbg_intounderlyingbytesource_free(t, 0);
    } catch (e) {
      s(e);
    }
  }
  get autoAllocateChunkSize() {
    if (this.__wbg_inst !== void 0 && this.__wbg_inst !== o) throw new Error("Invalid stale object from previous Wasm instance");
    let t;
    c();
    try {
      t = i.intounderlyingbytesource_autoAllocateChunkSize(this.__wbg_ptr);
    } catch (e) {
      s(e);
    }
    return t >>> 0;
  }
  cancel() {
    if (this.__wbg_inst !== void 0 && this.__wbg_inst !== o) throw new Error("Invalid stale object from previous Wasm instance");
    let t = this.__destroy_into_raw();
    c();
    try {
      i.intounderlyingbytesource_cancel(t);
    } catch (e) {
      s(e);
    }
  }
  pull(t) {
    if (this.__wbg_inst !== void 0 && this.__wbg_inst !== o) throw new Error("Invalid stale object from previous Wasm instance");
    let e;
    c();
    try {
      e = i.intounderlyingbytesource_pull(this.__wbg_ptr, t);
    } catch (n) {
      s(n);
    }
    return e;
  }
  start(t) {
    if (this.__wbg_inst !== void 0 && this.__wbg_inst !== o) throw new Error("Invalid stale object from previous Wasm instance");
    c();
    try {
      i.intounderlyingbytesource_start(this.__wbg_ptr, t);
    } catch (e) {
      s(e);
    }
  }
  get type() {
    if (this.__wbg_inst !== void 0 && this.__wbg_inst !== o) throw new Error("Invalid stale object from previous Wasm instance");
    let t;
    c();
    try {
      t = i.intounderlyingbytesource_type(this.__wbg_ptr);
    } catch (e) {
      s(e);
    }
    return at[t];
  }
};
Symbol.dispose && (R.prototype[Symbol.dispose] = R.prototype.free);
var F = class {
  static {
    __name(this, "F");
  }
  __destroy_into_raw() {
    let t = this.__wbg_ptr;
    return this.__wbg_ptr = 0, gt.unregister(this), t;
  }
  free() {
    let t = this.__destroy_into_raw();
    c();
    try {
      i.__wbg_intounderlyingsink_free(t, 0);
    } catch (e) {
      s(e);
    }
  }
  abort(t) {
    if (this.__wbg_inst !== void 0 && this.__wbg_inst !== o) throw new Error("Invalid stale object from previous Wasm instance");
    let e = this.__destroy_into_raw(), n;
    c();
    try {
      n = i.intounderlyingsink_abort(e, t);
    } catch (_) {
      s(_);
    }
    return n;
  }
  close() {
    if (this.__wbg_inst !== void 0 && this.__wbg_inst !== o) throw new Error("Invalid stale object from previous Wasm instance");
    let t = this.__destroy_into_raw(), e;
    c();
    try {
      e = i.intounderlyingsink_close(t);
    } catch (n) {
      s(n);
    }
    return e;
  }
  write(t) {
    if (this.__wbg_inst !== void 0 && this.__wbg_inst !== o) throw new Error("Invalid stale object from previous Wasm instance");
    let e;
    c();
    try {
      e = i.intounderlyingsink_write(this.__wbg_ptr, t);
    } catch (n) {
      s(n);
    }
    return e;
  }
};
Symbol.dispose && (F.prototype[Symbol.dispose] = F.prototype.free);
var S = class {
  static {
    __name(this, "S");
  }
  __destroy_into_raw() {
    let t = this.__wbg_ptr;
    return this.__wbg_ptr = 0, dt.unregister(this), t;
  }
  free() {
    let t = this.__destroy_into_raw();
    c();
    try {
      i.__wbg_intounderlyingsource_free(t, 0);
    } catch (e) {
      s(e);
    }
  }
  cancel() {
    if (this.__wbg_inst !== void 0 && this.__wbg_inst !== o) throw new Error("Invalid stale object from previous Wasm instance");
    let t = this.__destroy_into_raw();
    c();
    try {
      i.intounderlyingsource_cancel(t);
    } catch (e) {
      s(e);
    }
  }
  pull(t) {
    if (this.__wbg_inst !== void 0 && this.__wbg_inst !== o) throw new Error("Invalid stale object from previous Wasm instance");
    let e;
    c();
    try {
      e = i.intounderlyingsource_pull(this.__wbg_ptr, t);
    } catch (n) {
      s(n);
    }
    return e;
  }
};
Symbol.dispose && (S.prototype[Symbol.dispose] = S.prototype.free);
var x = class r {
  static {
    __name(this, "r");
  }
  static __wrap(t) {
    let e = Object.create(r.prototype);
    return e.__wbg_ptr = t, Object.defineProperty(e, "__wbg_inst", { value: o, writable: true }), Q.register(e, { ptr: t, instance: o }, e), e;
  }
  __destroy_into_raw() {
    let t = this.__wbg_ptr;
    return this.__wbg_ptr = 0, Q.unregister(this), t;
  }
  free() {
    let t = this.__destroy_into_raw();
    c();
    try {
      i.__wbg_minifyconfig_free(t, 0);
    } catch (e) {
      s(e);
    }
  }
  get css() {
    if (this.__wbg_inst !== void 0 && this.__wbg_inst !== o) throw new Error("Invalid stale object from previous Wasm instance");
    let t;
    c();
    try {
      t = i.__wbg_get_minifyconfig_css(this.__wbg_ptr);
    } catch (e) {
      s(e);
    }
    return t !== 0;
  }
  get html() {
    if (this.__wbg_inst !== void 0 && this.__wbg_inst !== o) throw new Error("Invalid stale object from previous Wasm instance");
    let t;
    c();
    try {
      t = i.__wbg_get_minifyconfig_html(this.__wbg_ptr);
    } catch (e) {
      s(e);
    }
    return t !== 0;
  }
  get js() {
    if (this.__wbg_inst !== void 0 && this.__wbg_inst !== o) throw new Error("Invalid stale object from previous Wasm instance");
    let t;
    c();
    try {
      t = i.__wbg_get_minifyconfig_js(this.__wbg_ptr);
    } catch (e) {
      s(e);
    }
    return t !== 0;
  }
  set css(t) {
    if (this.__wbg_inst !== void 0 && this.__wbg_inst !== o) throw new Error("Invalid stale object from previous Wasm instance");
    c();
    try {
      i.__wbg_set_minifyconfig_css(this.__wbg_ptr, t);
    } catch (e) {
      s(e);
    }
  }
  set html(t) {
    if (this.__wbg_inst !== void 0 && this.__wbg_inst !== o) throw new Error("Invalid stale object from previous Wasm instance");
    c();
    try {
      i.__wbg_set_minifyconfig_html(this.__wbg_ptr, t);
    } catch (e) {
      s(e);
    }
  }
  set js(t) {
    if (this.__wbg_inst !== void 0 && this.__wbg_inst !== o) throw new Error("Invalid stale object from previous Wasm instance");
    c();
    try {
      i.__wbg_set_minifyconfig_js(this.__wbg_ptr, t);
    } catch (e) {
      s(e);
    }
  }
};
Symbol.dispose && (x.prototype[Symbol.dispose] = x.prototype.free);
var j = class {
  static {
    __name(this, "j");
  }
  __destroy_into_raw() {
    let t = this.__wbg_ptr;
    return this.__wbg_ptr = 0, lt.unregister(this), t;
  }
  free() {
    let t = this.__destroy_into_raw();
    c();
    try {
      i.__wbg_r2range_free(t, 0);
    } catch (e) {
      s(e);
    }
  }
  get length() {
    if (this.__wbg_inst !== void 0 && this.__wbg_inst !== o) throw new Error("Invalid stale object from previous Wasm instance");
    let t;
    c();
    try {
      t = i.__wbg_get_r2range_length(this.__wbg_ptr);
    } catch (e) {
      s(e);
    }
    return t[0] === 0 ? void 0 : t[1];
  }
  get offset() {
    if (this.__wbg_inst !== void 0 && this.__wbg_inst !== o) throw new Error("Invalid stale object from previous Wasm instance");
    let t;
    c();
    try {
      t = i.__wbg_get_r2range_offset(this.__wbg_ptr);
    } catch (e) {
      s(e);
    }
    return t[0] === 0 ? void 0 : t[1];
  }
  get suffix() {
    if (this.__wbg_inst !== void 0 && this.__wbg_inst !== o) throw new Error("Invalid stale object from previous Wasm instance");
    let t;
    c();
    try {
      t = i.__wbg_get_r2range_suffix(this.__wbg_ptr);
    } catch (e) {
      s(e);
    }
    return t[0] === 0 ? void 0 : t[1];
  }
  set length(t) {
    if (this.__wbg_inst !== void 0 && this.__wbg_inst !== o) throw new Error("Invalid stale object from previous Wasm instance");
    c();
    try {
      i.__wbg_set_r2range_length(this.__wbg_ptr, !u(t), u(t) ? 0 : t);
    } catch (e) {
      s(e);
    }
  }
  set offset(t) {
    if (this.__wbg_inst !== void 0 && this.__wbg_inst !== o) throw new Error("Invalid stale object from previous Wasm instance");
    c();
    try {
      i.__wbg_set_r2range_offset(this.__wbg_ptr, !u(t), u(t) ? 0 : t);
    } catch (e) {
      s(e);
    }
  }
  set suffix(t) {
    if (this.__wbg_inst !== void 0 && this.__wbg_inst !== o) throw new Error("Invalid stale object from previous Wasm instance");
    c();
    try {
      i.__wbg_set_r2range_suffix(this.__wbg_ptr, !u(t), u(t) ? 0 : t);
    } catch (e) {
      s(e);
    }
  }
};
Symbol.dispose && (j.prototype[Symbol.dispose] = j.prototype.free);
function O() {
  o++, m = null, k = null, A = null, typeof numBytesDecoded < "u" && (numBytesDecoded = 0), typeof l < "u" && (l = 0), V = false, T = false, N = new WebAssembly.Instance(Z, rt()), i = N.exports, i.__wbindgen_start();
}
__name(O, "O");
function tt() {
  let r2;
  c();
  try {
    r2 = i.__worker_init_state();
  } catch (t) {
    s(t);
  }
  return r2;
}
__name(tt, "tt");
function et(r2, t, e) {
  let n;
  c();
  try {
    n = i.fetch(r2, t, e);
  } catch (_) {
    s(_);
  }
  return n;
}
__name(et, "et");
function nt() {
  c();
  try {
    i.init();
  } catch (r2) {
    s(r2);
  }
}
__name(nt, "nt");
function rt() {
  return { __proto__: null, "./index_bg.js": { __proto__: null, __wbg_Error_92b29b0548f8b746: /* @__PURE__ */ __name(function(t, e) {
    return Error(d(t, e));
  }, "__wbg_Error_92b29b0548f8b746"), __wbg_String_8564e559799eccda: /* @__PURE__ */ __name(function(t, e) {
    let n = String(e), _ = v(n, i.__wbindgen_malloc, i.__wbindgen_realloc), a = l;
    w().setInt32(t + 4, a, true), w().setInt32(t + 0, _, true);
  }, "__wbg_String_8564e559799eccda"), __wbg___wbindgen_boolean_get_fa956cfa2d1bd751: /* @__PURE__ */ __name(function(t) {
    let e = t, n = typeof e == "boolean" ? e : void 0;
    return u(n) ? 16777215 : n ? 1 : 0;
  }, "__wbg___wbindgen_boolean_get_fa956cfa2d1bd751"), __wbg___wbindgen_debug_string_c25d447a39f5578f: /* @__PURE__ */ __name(function(t, e) {
    let n = B(e), _ = v(n, i.__wbindgen_malloc, i.__wbindgen_realloc), a = l;
    w().setInt32(t + 4, a, true), w().setInt32(t + 0, _, true);
  }, "__wbg___wbindgen_debug_string_c25d447a39f5578f"), __wbg___wbindgen_is_function_1ff95bcc5517c252: /* @__PURE__ */ __name(function(t) {
    return typeof t == "function";
  }, "__wbg___wbindgen_is_function_1ff95bcc5517c252"), __wbg___wbindgen_is_null_or_undefined_f39ff01e68a1554f: /* @__PURE__ */ __name(function(t) {
    return t == null;
  }, "__wbg___wbindgen_is_null_or_undefined_f39ff01e68a1554f"), __wbg___wbindgen_is_object_a27215656b807791: /* @__PURE__ */ __name(function(t) {
    let e = t;
    return typeof e == "object" && e !== null;
  }, "__wbg___wbindgen_is_object_a27215656b807791"), __wbg___wbindgen_is_string_ea5e6cc2e4141dfe: /* @__PURE__ */ __name(function(t) {
    return typeof t == "string";
  }, "__wbg___wbindgen_is_string_ea5e6cc2e4141dfe"), __wbg___wbindgen_is_undefined_c05833b95a3cf397: /* @__PURE__ */ __name(function(t) {
    return t === void 0;
  }, "__wbg___wbindgen_is_undefined_c05833b95a3cf397"), __wbg___wbindgen_jsval_loose_eq_db4c3b15f63fc170: /* @__PURE__ */ __name(function(t, e) {
    return t == e;
  }, "__wbg___wbindgen_jsval_loose_eq_db4c3b15f63fc170"), __wbg___wbindgen_number_get_394265ed1e1b84ee: /* @__PURE__ */ __name(function(t, e) {
    let n = e, _ = typeof n == "number" ? n : void 0;
    w().setFloat64(t + 8, u(_) ? 0 : _, true), w().setInt32(t + 0, !u(_), true);
  }, "__wbg___wbindgen_number_get_394265ed1e1b84ee"), __wbg___wbindgen_reinit_ecd4a8d70c8ce492: /* @__PURE__ */ __name(function() {
    T = true;
  }, "__wbg___wbindgen_reinit_ecd4a8d70c8ce492"), __wbg___wbindgen_string_get_b0ca35b86a603356: /* @__PURE__ */ __name(function(t, e) {
    let n = e, _ = typeof n == "string" ? n : void 0;
    var a = u(_) ? 0 : v(_, i.__wbindgen_malloc, i.__wbindgen_realloc), f = l;
    w().setInt32(t + 4, f, true), w().setInt32(t + 0, a, true);
  }, "__wbg___wbindgen_string_get_b0ca35b86a603356"), __wbg___wbindgen_throw_344f42d3211c4765: /* @__PURE__ */ __name(function(t, e) {
    throw new WebAssembly.Exception(q, [new Error(d(t, e))]);
  }, "__wbg___wbindgen_throw_344f42d3211c4765"), __wbg__wbg_cb_unref_fffb441def202758: /* @__PURE__ */ __name(function(t) {
    t._wbg_cb_unref();
  }, "__wbg__wbg_cb_unref_fffb441def202758"), __wbg_body_18c9f2ac15ead4b2: /* @__PURE__ */ __name(function(t) {
    let e = t.body;
    return u(e) ? 0 : h(e);
  }, "__wbg_body_18c9f2ac15ead4b2"), __wbg_buffer_54b87055582c8a81: /* @__PURE__ */ __name(function(t) {
    return t.buffer;
  }, "__wbg_buffer_54b87055582c8a81"), __wbg_byobRequest_06b654bb15590436: /* @__PURE__ */ __name(function(t) {
    let e = t.byobRequest;
    return u(e) ? 0 : h(e);
  }, "__wbg_byobRequest_06b654bb15590436"), __wbg_byteLength_41862ca4020b9c43: /* @__PURE__ */ __name(function(t) {
    return t.byteLength;
  }, "__wbg_byteLength_41862ca4020b9c43"), __wbg_byteOffset_d42e18c4441f628b: /* @__PURE__ */ __name(function(t) {
    return t.byteOffset;
  }, "__wbg_byteOffset_d42e18c4441f628b"), __wbg_call_8a2dd23819f8a60a: /* @__PURE__ */ __name(function(t, e) {
    return t.call(e);
  }, "__wbg_call_8a2dd23819f8a60a"), __wbg_call_a6e5c5dce5018821: /* @__PURE__ */ __name(function(t, e, n) {
    return t.call(e, n);
  }, "__wbg_call_a6e5c5dce5018821"), __wbg_cancel_3983a93e24cc66b3: /* @__PURE__ */ __name(function(t) {
    return t.cancel();
  }, "__wbg_cancel_3983a93e24cc66b3"), __wbg_catch_c1a60df4c30d76d3: /* @__PURE__ */ __name(function(t, e) {
    return t.catch(e);
  }, "__wbg_catch_c1a60df4c30d76d3"), __wbg_cause_5eb2f50e6f22fd6c: /* @__PURE__ */ __name(function(t) {
    return t.cause;
  }, "__wbg_cause_5eb2f50e6f22fd6c"), __wbg_cf_0214917b05eb22e8: /* @__PURE__ */ __name(function(t) {
    let e = t.cf;
    return u(e) ? 0 : h(e);
  }, "__wbg_cf_0214917b05eb22e8"), __wbg_cf_a20029b537f86569: /* @__PURE__ */ __name(function(t) {
    let e = t.cf;
    return u(e) ? 0 : h(e);
  }, "__wbg_cf_a20029b537f86569"), __wbg_close_249a23304523681b: /* @__PURE__ */ __name(function(t) {
    t.close();
  }, "__wbg_close_249a23304523681b"), __wbg_close_72d318d9c16e83ef: /* @__PURE__ */ __name(function(t) {
    t.close();
  }, "__wbg_close_72d318d9c16e83ef"), __wbg_constructor_07a76432cc61e2e7: /* @__PURE__ */ __name(function(t) {
    return t.constructor;
  }, "__wbg_constructor_07a76432cc61e2e7"), __wbg_crypto_38df2bab126b63dc: /* @__PURE__ */ __name(function(t) {
    return t.crypto;
  }, "__wbg_crypto_38df2bab126b63dc"), __wbg_delete_8216f5ee1b1aca49: /* @__PURE__ */ __name(function(t, e, n) {
    return t.delete(d(e, n));
  }, "__wbg_delete_8216f5ee1b1aca49"), __wbg_done_89b2b13e91a60321: /* @__PURE__ */ __name(function(t) {
    return t.done;
  }, "__wbg_done_89b2b13e91a60321"), __wbg_enqueue_6d83b4c6281bafd6: /* @__PURE__ */ __name(function(t, e) {
    t.enqueue(e);
  }, "__wbg_enqueue_6d83b4c6281bafd6"), __wbg_error_744744ff0c9861e6: /* @__PURE__ */ __name(function(t) {
    console.error(t);
  }, "__wbg_error_744744ff0c9861e6"), __wbg_error_7ed559cd7146b49d: /* @__PURE__ */ __name(function(t, e) {
    console.error(t, e);
  }, "__wbg_error_7ed559cd7146b49d"), __wbg_fetch_a730576a617dd701: /* @__PURE__ */ __name(function(t, e) {
    return t.fetch(e);
  }, "__wbg_fetch_a730576a617dd701"), __wbg_getRandomValues_c44a50d8cfdaebeb: /* @__PURE__ */ __name(function(t, e) {
    t.getRandomValues(e);
  }, "__wbg_getRandomValues_c44a50d8cfdaebeb"), __wbg_getReader_bb0230851fbf986b: /* @__PURE__ */ __name(function(t) {
    return t.getReader();
  }, "__wbg_getReader_bb0230851fbf986b"), __wbg_getTime_d6f070c088c9b5ed: /* @__PURE__ */ __name(function(t) {
    return t.getTime();
  }, "__wbg_getTime_d6f070c088c9b5ed"), __wbg_get_07a843c77ca6efba: /* @__PURE__ */ __name(function(t, e, n) {
    return t.get(d(e, n));
  }, "__wbg_get_07a843c77ca6efba"), __wbg_get_18e0163e38e5048d: /* @__PURE__ */ __name(function(t, e, n, _) {
    let a = e.get(d(n, _));
    var f = u(a) ? 0 : v(a, i.__wbindgen_malloc, i.__wbindgen_realloc), b = l;
    w().setInt32(t + 4, b, true), w().setInt32(t + 0, f, true);
  }, "__wbg_get_18e0163e38e5048d"), __wbg_get_507a50627bffa49b: /* @__PURE__ */ __name(function(t, e) {
    return t[e >>> 0];
  }, "__wbg_get_507a50627bffa49b"), __wbg_get_78f252d074a84d0b: /* @__PURE__ */ __name(function(t, e) {
    return Reflect.get(t, e);
  }, "__wbg_get_78f252d074a84d0b"), __wbg_get_8f0e18b878d0fab8: /* @__PURE__ */ __name(function(t, e) {
    let n = Reflect.get(t, e);
    return u(n) ? 0 : h(n);
  }, "__wbg_get_8f0e18b878d0fab8"), __wbg_get_be757f0f312c4319: /* @__PURE__ */ __name(function(t, e) {
    return t.get(e);
  }, "__wbg_get_be757f0f312c4319"), __wbg_get_c7eb1f358a7654df: /* @__PURE__ */ __name(function(t, e) {
    return Reflect.get(t, e);
  }, "__wbg_get_c7eb1f358a7654df"), __wbg_get_done_670108eb06ecbe46: /* @__PURE__ */ __name(function(t) {
    let e = t.done;
    return u(e) ? 16777215 : e ? 1 : 0;
  }, "__wbg_get_done_670108eb06ecbe46"), __wbg_get_value_f465f5be30aa0963: /* @__PURE__ */ __name(function(t) {
    return t.value;
  }, "__wbg_get_value_f465f5be30aa0963"), __wbg_headers_7b59c5203c8c475d: /* @__PURE__ */ __name(function(t) {
    return t.headers;
  }, "__wbg_headers_7b59c5203c8c475d"), __wbg_headers_cf9c80f30e2a4eff: /* @__PURE__ */ __name(function(t) {
    return t.headers;
  }, "__wbg_headers_cf9c80f30e2a4eff"), __wbg_idFromName_f87c89f5c31a83a6: /* @__PURE__ */ __name(function(t, e, n) {
    return t.idFromName(d(e, n));
  }, "__wbg_idFromName_f87c89f5c31a83a6"), __wbg_instanceId_27a130234331eb08: /* @__PURE__ */ __name(function(t) {
    return t.instanceId;
  }, "__wbg_instanceId_27a130234331eb08"), __wbg_instanceof_ArrayBuffer_4480b9e0068a8adb: /* @__PURE__ */ __name(function(t) {
    let e;
    try {
      e = t instanceof ArrayBuffer;
    } catch {
      e = false;
    }
    return e;
  }, "__wbg_instanceof_ArrayBuffer_4480b9e0068a8adb"), __wbg_instanceof_Error_1fdac9f13a8181ba: /* @__PURE__ */ __name(function(t) {
    let e;
    try {
      e = t instanceof Error;
    } catch {
      e = false;
    }
    return e;
  }, "__wbg_instanceof_Error_1fdac9f13a8181ba"), __wbg_instanceof_Map_e5b5e3db98422fcc: /* @__PURE__ */ __name(function(t) {
    let e;
    try {
      e = t instanceof Map;
    } catch {
      e = false;
    }
    return e;
  }, "__wbg_instanceof_Map_e5b5e3db98422fcc"), __wbg_instanceof_ReadableStream_3bdc7d10b03fd402: /* @__PURE__ */ __name(function(t) {
    let e;
    try {
      e = t instanceof ReadableStream;
    } catch {
      e = false;
    }
    return e;
  }, "__wbg_instanceof_ReadableStream_3bdc7d10b03fd402"), __wbg_instanceof_Response_c8b64b2256f01bec: /* @__PURE__ */ __name(function(t) {
    let e;
    try {
      e = t instanceof Response;
    } catch {
      e = false;
    }
    return e;
  }, "__wbg_instanceof_Response_c8b64b2256f01bec"), __wbg_instanceof_Uint8Array_309b927aaf7a3fc7: /* @__PURE__ */ __name(function(t) {
    let e;
    try {
      e = t instanceof Uint8Array;
    } catch {
      e = false;
    }
    return e;
  }, "__wbg_instanceof_Uint8Array_309b927aaf7a3fc7"), __wbg_iterator_6f722e4a93058b71: /* @__PURE__ */ __name(function() {
    return Symbol.iterator;
  }, "__wbg_iterator_6f722e4a93058b71"), __wbg_keys_15ebe487809c60dd: /* @__PURE__ */ __name(function(t) {
    return t.keys();
  }, "__wbg_keys_15ebe487809c60dd"), __wbg_length_1f0964f4a5e2c6d8: /* @__PURE__ */ __name(function(t) {
    return t.length;
  }, "__wbg_length_1f0964f4a5e2c6d8"), __wbg_list_301f5869c6f824db: /* @__PURE__ */ __name(function(t, e) {
    return t.list(e);
  }, "__wbg_list_301f5869c6f824db"), __wbg_message_8326fb1d549bebc5: /* @__PURE__ */ __name(function(t) {
    return t.message;
  }, "__wbg_message_8326fb1d549bebc5"), __wbg_method_b84982a2d750a05f: /* @__PURE__ */ __name(function(t, e) {
    let n = e.method, _ = v(n, i.__wbindgen_malloc, i.__wbindgen_realloc), a = l;
    w().setInt32(t + 4, a, true), w().setInt32(t + 0, _, true);
  }, "__wbg_method_b84982a2d750a05f"), __wbg_minifyconfig_new: /* @__PURE__ */ __name(function(t) {
    return x.__wrap(t);
  }, "__wbg_minifyconfig_new"), __wbg_msCrypto_bd5a034af96bcba6: /* @__PURE__ */ __name(function(t) {
    return t.msCrypto;
  }, "__wbg_msCrypto_bd5a034af96bcba6"), __wbg_name_b0b4809690944614: /* @__PURE__ */ __name(function(t) {
    return t.name;
  }, "__wbg_name_b0b4809690944614"), __wbg_name_f4eafd9ffc22cda1: /* @__PURE__ */ __name(function(t) {
    return t.name;
  }, "__wbg_name_f4eafd9ffc22cda1"), __wbg_new_0_3da9e97f24fc69be: /* @__PURE__ */ __name(function() {
    return /* @__PURE__ */ new Date();
  }, "__wbg_new_0_3da9e97f24fc69be"), __wbg_new_0d809930cd1354c6: /* @__PURE__ */ __name(function() {
    return new Headers();
  }, "__wbg_new_0d809930cd1354c6"), __wbg_new_7796ffc7ed656783: /* @__PURE__ */ __name(function() {
    return /* @__PURE__ */ new Map();
  }, "__wbg_new_7796ffc7ed656783"), __wbg_new_b667d279fd5aa943: /* @__PURE__ */ __name(function(t, e) {
    return new Error(d(t, e));
  }, "__wbg_new_b667d279fd5aa943"), __wbg_new_cd45aabdf6073e84: /* @__PURE__ */ __name(function(t) {
    return new Uint8Array(t);
  }, "__wbg_new_cd45aabdf6073e84"), __wbg_new_da52cf8fe3429cb2: /* @__PURE__ */ __name(function() {
    return new Object();
  }, "__wbg_new_da52cf8fe3429cb2"), __wbg_new_typed_1824d93f294193e5: /* @__PURE__ */ __name(function(t, e) {
    try {
      var n = { a: t, b: e }, _ = /* @__PURE__ */ __name((f, b) => {
        let g = n.a;
        n.a = 0;
        try {
          return ct(g, n.b, f, b);
        } finally {
          n.a = g;
        }
      }, "_");
      return new Promise(_);
    } finally {
      n.a = 0;
    }
  }, "__wbg_new_typed_1824d93f294193e5"), __wbg_new_with_byte_offset_and_length_54c7724ee3ec7d82: /* @__PURE__ */ __name(function(t, e, n) {
    return new Uint8Array(t, e >>> 0, n >>> 0);
  }, "__wbg_new_with_byte_offset_and_length_54c7724ee3ec7d82"), __wbg_new_with_length_e6785c33c8e4cce8: /* @__PURE__ */ __name(function(t) {
    return new Uint8Array(t >>> 0);
  }, "__wbg_new_with_length_e6785c33c8e4cce8"), __wbg_new_with_opt_buffer_source_and_init_c8d6537f14c8efb5: /* @__PURE__ */ __name(function(t, e) {
    return new Response(t, e);
  }, "__wbg_new_with_opt_buffer_source_and_init_c8d6537f14c8efb5"), __wbg_new_with_opt_readable_stream_and_init_bfd13e2b23ff2e0c: /* @__PURE__ */ __name(function(t, e) {
    return new Response(t, e);
  }, "__wbg_new_with_opt_readable_stream_and_init_bfd13e2b23ff2e0c"), __wbg_new_with_opt_str_and_init_74529c37934bd6b4: /* @__PURE__ */ __name(function(t, e, n) {
    return new Response(t === 0 ? void 0 : d(t, e), n);
  }, "__wbg_new_with_opt_str_and_init_74529c37934bd6b4"), __wbg_new_with_str_and_init_d95cbe11ce28e65e: /* @__PURE__ */ __name(function(t, e, n) {
    return new Request(d(t, e), n);
  }, "__wbg_new_with_str_and_init_d95cbe11ce28e65e"), __wbg_next_6dbf2c0ac8cde20f: /* @__PURE__ */ __name(function(t) {
    return t.next;
  }, "__wbg_next_6dbf2c0ac8cde20f"), __wbg_next_71f2aa1cb3d1e37e: /* @__PURE__ */ __name(function(t) {
    return t.next();
  }, "__wbg_next_71f2aa1cb3d1e37e"), __wbg_node_84ea875411254db1: /* @__PURE__ */ __name(function(t) {
    return t.node;
  }, "__wbg_node_84ea875411254db1"), __wbg_process_44c7a14e11e9f69e: /* @__PURE__ */ __name(function(t) {
    return t.process;
  }, "__wbg_process_44c7a14e11e9f69e"), __wbg_prototypesetcall_4770620bbe4688a0: /* @__PURE__ */ __name(function(t, e, n) {
    Uint8Array.prototype.set.call(L(t, e), n);
  }, "__wbg_prototypesetcall_4770620bbe4688a0"), __wbg_put_0cbeed0b6e2ac746: /* @__PURE__ */ __name(function(t, e, n, _) {
    return t.put(d(e, n), _);
  }, "__wbg_put_0cbeed0b6e2ac746"), __wbg_queueMicrotask_0ab5b2d2393e99b9: /* @__PURE__ */ __name(function(t) {
    return t.queueMicrotask;
  }, "__wbg_queueMicrotask_0ab5b2d2393e99b9"), __wbg_queueMicrotask_6a09b7bc46549209: /* @__PURE__ */ __name(function(t) {
    queueMicrotask(t);
  }, "__wbg_queueMicrotask_6a09b7bc46549209"), __wbg_randomFillSync_6c25eac9869eb53c: /* @__PURE__ */ __name(function(t, e) {
    t.randomFillSync(e);
  }, "__wbg_randomFillSync_6c25eac9869eb53c"), __wbg_read_8afa15f12a160ef8: /* @__PURE__ */ __name(function(t) {
    return t.read();
  }, "__wbg_read_8afa15f12a160ef8"), __wbg_releaseLock_5b92874cad775644: /* @__PURE__ */ __name(function(t) {
    t.releaseLock();
  }, "__wbg_releaseLock_5b92874cad775644"), __wbg_require_b4edbdcf3e2a1ef0: /* @__PURE__ */ __name(function() {
    return module.require;
  }, "__wbg_require_b4edbdcf3e2a1ef0"), __wbg_resolve_2191a4dfe481c25b: /* @__PURE__ */ __name(function(t) {
    return Promise.resolve(t);
  }, "__wbg_resolve_2191a4dfe481c25b"), __wbg_respond_510e32df8aeb6817: /* @__PURE__ */ __name(function(t, e) {
    t.respond(e >>> 0);
  }, "__wbg_respond_510e32df8aeb6817"), __wbg_set_0de9c62c23d04ad5: /* @__PURE__ */ __name(function(t, e, n, _, a) {
    t.set(d(e, n), d(_, a));
  }, "__wbg_set_0de9c62c23d04ad5"), __wbg_set_4d7dd76f3dae2926: /* @__PURE__ */ __name(function(t, e, n) {
    t.set(L(e, n));
  }, "__wbg_set_4d7dd76f3dae2926"), __wbg_set_575dd786d51585f8: /* @__PURE__ */ __name(function(t, e, n) {
    return t.set(e, n);
  }, "__wbg_set_575dd786d51585f8"), __wbg_set_6be42768c690e380: /* @__PURE__ */ __name(function(t, e, n) {
    t[e] = n;
  }, "__wbg_set_6be42768c690e380"), __wbg_set_8535240470bf2500: /* @__PURE__ */ __name(function(t, e, n) {
    return Reflect.set(t, e, n);
  }, "__wbg_set_8535240470bf2500"), __wbg_set_body_029f2d171e0a005f: /* @__PURE__ */ __name(function(t, e) {
    t.body = e;
  }, "__wbg_set_body_029f2d171e0a005f"), __wbg_set_cache_b4a740b195c051f4: /* @__PURE__ */ __name(function(t, e) {
    t.cache = ft[e];
  }, "__wbg_set_cache_b4a740b195c051f4"), __wbg_set_criticalError_0d391f2a45c3bc42: /* @__PURE__ */ __name(function(t, e) {
    t.criticalError = e !== 0;
  }, "__wbg_set_criticalError_0d391f2a45c3bc42"), __wbg_set_headers_4be66d6f175ce615: /* @__PURE__ */ __name(function(t, e) {
    t.headers = e;
  }, "__wbg_set_headers_4be66d6f175ce615"), __wbg_set_headers_9c61d123c3ee1f10: /* @__PURE__ */ __name(function(t, e) {
    t.headers = e;
  }, "__wbg_set_headers_9c61d123c3ee1f10"), __wbg_set_instanceId_9b3420954adec865: /* @__PURE__ */ __name(function(t, e) {
    t.instanceId = e >>> 0;
  }, "__wbg_set_instanceId_9b3420954adec865"), __wbg_set_method_5532d59b92d76467: /* @__PURE__ */ __name(function(t, e, n) {
    t.method = d(e, n);
  }, "__wbg_set_method_5532d59b92d76467"), __wbg_set_redirect_badd73a0bcb765e3: /* @__PURE__ */ __name(function(t, e) {
    t.redirect = ut[e];
  }, "__wbg_set_redirect_badd73a0bcb765e3"), __wbg_set_status_3593dfcb55e7ee3c: /* @__PURE__ */ __name(function(t, e) {
    t.status = e;
  }, "__wbg_set_status_3593dfcb55e7ee3c"), __wbg_size_56cb46b36f9b2664: /* @__PURE__ */ __name(function(t) {
    return t.size;
  }, "__wbg_size_56cb46b36f9b2664"), __wbg_static_accessor_GLOBAL_4ef717fb391d88b7: /* @__PURE__ */ __name(function() {
    let t = typeof global > "u" ? null : global;
    return u(t) ? 0 : h(t);
  }, "__wbg_static_accessor_GLOBAL_4ef717fb391d88b7"), __wbg_static_accessor_GLOBAL_THIS_8d1badc68b5a74f4: /* @__PURE__ */ __name(function() {
    let t = typeof globalThis > "u" ? null : globalThis;
    return u(t) ? 0 : h(t);
  }, "__wbg_static_accessor_GLOBAL_THIS_8d1badc68b5a74f4"), __wbg_static_accessor_INIT_STATE_35966a05809b7176: /* @__PURE__ */ __name(function() {
    return G;
  }, "__wbg_static_accessor_INIT_STATE_35966a05809b7176"), __wbg_static_accessor_SELF_146583524fe1469b: /* @__PURE__ */ __name(function() {
    let t = typeof self > "u" ? null : self;
    return u(t) ? 0 : h(t);
  }, "__wbg_static_accessor_SELF_146583524fe1469b"), __wbg_static_accessor_WINDOW_f2829a2234d7819e: /* @__PURE__ */ __name(function() {
    let t = typeof window > "u" ? null : window;
    return u(t) ? 0 : h(t);
  }, "__wbg_static_accessor_WINDOW_f2829a2234d7819e"), __wbg_status_c45b3b9b3033184a: /* @__PURE__ */ __name(function(t) {
    return t.status;
  }, "__wbg_status_c45b3b9b3033184a"), __wbg_storage_63bc3c0e560235be: /* @__PURE__ */ __name(function(t) {
    return t.storage;
  }, "__wbg_storage_63bc3c0e560235be"), __wbg_subarray_3ed232c8a6baee09: /* @__PURE__ */ __name(function(t, e, n) {
    return t.subarray(e >>> 0, n >>> 0);
  }, "__wbg_subarray_3ed232c8a6baee09"), __wbg_text_67a40fc1604b6d39: /* @__PURE__ */ __name(function(t) {
    return t.text();
  }, "__wbg_text_67a40fc1604b6d39"), __wbg_then_16d107c451e9905d: /* @__PURE__ */ __name(function(t, e, n) {
    return t.then(e, n);
  }, "__wbg_then_16d107c451e9905d"), __wbg_then_6ec10ae38b3e92f7: /* @__PURE__ */ __name(function(t, e) {
    return t.then(e);
  }, "__wbg_then_6ec10ae38b3e92f7"), __wbg_url_f6cd241d61f89b82: /* @__PURE__ */ __name(function(t, e) {
    let n = e.url, _ = v(n, i.__wbindgen_malloc, i.__wbindgen_realloc), a = l;
    w().setInt32(t + 4, a, true), w().setInt32(t + 0, _, true);
  }, "__wbg_url_f6cd241d61f89b82"), __wbg_value_a5d5488a9589444a: /* @__PURE__ */ __name(function(t) {
    return t.value;
  }, "__wbg_value_a5d5488a9589444a"), __wbg_versions_276b2795b1c6a219: /* @__PURE__ */ __name(function(t) {
    return t.versions;
  }, "__wbg_versions_276b2795b1c6a219"), __wbg_view_21f1d4a4f175dfa9: /* @__PURE__ */ __name(function(t) {
    let e = t.view;
    return u(e) ? 0 : h(e);
  }, "__wbg_view_21f1d4a4f175dfa9"), __wbg_webSocket_d0b2797319952d03: /* @__PURE__ */ __name(function(t) {
    let e = t.webSocket;
    return u(e) ? 0 : h(e);
  }, "__wbg_webSocket_d0b2797319952d03"), __wbindgen_cast_0000000000000001: /* @__PURE__ */ __name(function(t, e) {
    return Y(t, e, ot);
  }, "__wbindgen_cast_0000000000000001"), __wbindgen_cast_0000000000000002: /* @__PURE__ */ __name(function(t, e) {
    return Y(t, e, st);
  }, "__wbindgen_cast_0000000000000002"), __wbindgen_cast_0000000000000003: /* @__PURE__ */ __name(function(t) {
    return t;
  }, "__wbindgen_cast_0000000000000003"), __wbindgen_cast_0000000000000004: /* @__PURE__ */ __name(function(t, e) {
    return L(t, e);
  }, "__wbindgen_cast_0000000000000004"), __wbindgen_cast_0000000000000005: /* @__PURE__ */ __name(function(t, e) {
    return d(t, e);
  }, "__wbindgen_cast_0000000000000005"), __wbindgen_cast_0000000000000006: /* @__PURE__ */ __name(function(t) {
    return BigInt.asUintN(64, t);
  }, "__wbindgen_cast_0000000000000006"), __wbindgen_init_externref_table: /* @__PURE__ */ __name(function() {
    let t = i.__wbindgen_externrefs, e = t.grow(4);
    t.set(0, void 0), t.set(e + 0, void 0), t.set(e + 1, null), t.set(e + 2, true), t.set(e + 3, false);
  }, "__wbindgen_init_externref_table"), __wbindgen_jstag: WebAssembly.JSTag, __wbindgen_rethrow_critical: /* @__PURE__ */ __name(function(t) {
    throw new Error("Critical error", { cause: t });
  }, "__wbindgen_rethrow_critical") } };
}
__name(rt, "rt");
var q = new WebAssembly.Tag({ parameters: ["externref"] });
var D;
var V = false;
function _t() {
  V = true;
  try {
    let r2 = $()[i.__abort_handler.value / 4];
    r2 && i.__wbindgen_export.get(r2)();
  } catch {
  }
}
__name(_t, "_t");
function s(r2) {
  throw r2 instanceof WebAssembly.Exception && r2.is(q) ? r2.getArg(q, 0) : ($()[D] = 1, _t(), r2);
}
__name(s, "s");
function c() {
  if (D ??= i.__instance_terminated.value / 4, $()[D]) {
    if (V || _t(), T) {
      O();
      return;
    }
    throw new Error("Module terminated");
  } else T && O();
}
__name(c, "c");
function ot(r2, t, e) {
  c();
  try {
    i.wasm_bindgen__convert__closures_____invoke__heb53e527852f00e3(r2, t, e);
  } catch (n) {
    s(n);
  }
}
__name(ot, "ot");
function st(r2, t, e) {
  let n;
  c();
  try {
    n = i.wasm_bindgen__convert__closures_____invoke__h7a2772d74aff0994(r2, t, e);
  } catch (_) {
    s(_);
  }
  if (n[1]) throw yt(n[0]);
}
__name(st, "st");
function ct(r2, t, e, n) {
  c();
  try {
    i.wasm_bindgen__convert__closures_____invoke__h46f0ab9679300f14(r2, t, e, n);
  } catch (_) {
    s(_);
  }
}
__name(ct, "ct");
var at = ["bytes"];
var ft = ["default", "no-store", "reload", "no-cache", "force-cache", "only-if-cached"];
var ut = ["follow", "error", "manual"];
var o = 0;
var bt = typeof FinalizationRegistry > "u" ? { register: /* @__PURE__ */ __name(() => {
}, "register"), unregister: /* @__PURE__ */ __name(() => {
}, "unregister") } : new FinalizationRegistry(({ ptr: r2, instance: t }) => {
  t === o && i.__wbg_containerstartupoptions_free(r2, 1);
});
var K = typeof FinalizationRegistry > "u" ? { register: /* @__PURE__ */ __name(() => {
}, "register"), unregister: /* @__PURE__ */ __name(() => {
}, "unregister") } : new FinalizationRegistry(({ ptr: r2, instance: t }) => {
  t === o && i.__wbg_hive_free(r2, 1);
});
var wt = typeof FinalizationRegistry > "u" ? { register: /* @__PURE__ */ __name(() => {
}, "register"), unregister: /* @__PURE__ */ __name(() => {
}, "unregister") } : new FinalizationRegistry(({ ptr: r2, instance: t }) => {
  t === o && i.__wbg_intounderlyingbytesource_free(r2, 1);
});
var gt = typeof FinalizationRegistry > "u" ? { register: /* @__PURE__ */ __name(() => {
}, "register"), unregister: /* @__PURE__ */ __name(() => {
}, "unregister") } : new FinalizationRegistry(({ ptr: r2, instance: t }) => {
  t === o && i.__wbg_intounderlyingsink_free(r2, 1);
});
var dt = typeof FinalizationRegistry > "u" ? { register: /* @__PURE__ */ __name(() => {
}, "register"), unregister: /* @__PURE__ */ __name(() => {
}, "unregister") } : new FinalizationRegistry(({ ptr: r2, instance: t }) => {
  t === o && i.__wbg_intounderlyingsource_free(r2, 1);
});
var Q = typeof FinalizationRegistry > "u" ? { register: /* @__PURE__ */ __name(() => {
}, "register"), unregister: /* @__PURE__ */ __name(() => {
}, "unregister") } : new FinalizationRegistry(({ ptr: r2, instance: t }) => {
  t === o && i.__wbg_minifyconfig_free(r2, 1);
});
var lt = typeof FinalizationRegistry > "u" ? { register: /* @__PURE__ */ __name(() => {
}, "register"), unregister: /* @__PURE__ */ __name(() => {
}, "unregister") } : new FinalizationRegistry(({ ptr: r2, instance: t }) => {
  t === o && i.__wbg_r2range_free(r2, 1);
});
function h(r2) {
  let t = i.__externref_table_alloc();
  return i.__wbindgen_externrefs.set(t, r2), t;
}
__name(h, "h");
var X = typeof FinalizationRegistry > "u" ? { register: /* @__PURE__ */ __name(() => {
}, "register"), unregister: /* @__PURE__ */ __name(() => {
}, "unregister") } : new FinalizationRegistry((r2) => {
  r2.instance === o && i.__wbindgen_destroy_closure(r2.a, r2.b);
});
function B(r2) {
  let t = typeof r2;
  if (t == "number" || t == "boolean" || r2 == null) return `${r2}`;
  if (t == "string") return `"${r2}"`;
  if (t == "symbol") {
    let _ = r2.description;
    return _ == null ? "Symbol" : `Symbol(${_})`;
  }
  if (t == "function") {
    let _ = r2.name;
    return typeof _ == "string" && _.length > 0 ? `Function(${_})` : "Function";
  }
  if (Array.isArray(r2)) {
    let _ = r2.length, a = "[";
    _ > 0 && (a += B(r2[0]));
    for (let f = 1; f < _; f++) a += ", " + B(r2[f]);
    return a += "]", a;
  }
  let e = /\[object ([^\]]+)\]/.exec(toString.call(r2)), n;
  if (e && e.length > 1) n = e[1];
  else return toString.call(r2);
  if (n == "Object") try {
    return "Object(" + JSON.stringify(r2) + ")";
  } catch {
    return "Object";
  }
  return r2 instanceof Error ? `${r2.name}: ${r2.message}
${r2.stack}` : n;
}
__name(B, "B");
function ht(r2, t) {
  r2 = r2 >>> 0;
  let e = w(), n = [];
  for (let _ = r2; _ < r2 + 4 * t; _ += 4) n.push(i.__wbindgen_externrefs.get(e.getUint32(_, true)));
  return i.__externref_drop_slice(r2, t), n;
}
__name(ht, "ht");
function L(r2, t) {
  return r2 = r2 >>> 0, z().subarray(r2 / 1, r2 / 1 + t);
}
__name(L, "L");
var m = null;
function w() {
  return (m === null || m.buffer.detached === true || m.buffer.detached === void 0 && m.buffer !== i.memory.buffer) && (m = new DataView(i.memory.buffer)), m;
}
__name(w, "w");
var k = null;
function $() {
  return (k === null || k.byteLength === 0) && (k = new Int32Array(i.memory.buffer)), k;
}
__name($, "$");
function d(r2, t) {
  return mt(r2 >>> 0, t);
}
__name(d, "d");
var A = null;
function z() {
  return (A === null || A.byteLength === 0) && (A = new Uint8Array(i.memory.buffer)), A;
}
__name(z, "z");
function u(r2) {
  return r2 == null;
}
__name(u, "u");
function Y(r2, t, e) {
  let n = { a: r2, b: t, cnt: 1, instance: o }, _ = /* @__PURE__ */ __name((...a) => {
    if (n.instance !== o) throw new Error("Cannot invoke closure from previous WASM instance");
    n.cnt++;
    let f = n.a;
    n.a = 0;
    try {
      return e(f, n.b, ...a);
    } finally {
      n.a = f, _._wbg_cb_unref();
    }
  }, "_");
  return _._wbg_cb_unref = () => {
    --n.cnt === 0 && (i.__wbindgen_destroy_closure(n.a, n.b), n.a = 0, X.unregister(n));
  }, X.register(_, n, n), _;
}
__name(Y, "Y");
function pt(r2, t) {
  let e = t(r2.length * 4, 4) >>> 0;
  for (let n = 0; n < r2.length; n++) {
    let _ = h(r2[n]);
    w().setUint32(e + 4 * n, _, true);
  }
  return l = r2.length, e;
}
__name(pt, "pt");
function v(r2, t, e) {
  if (e === void 0) {
    let b = P.encode(r2), g = t(b.length, 1) >>> 0;
    return z().subarray(g, g + b.length).set(b), l = b.length, g;
  }
  let n = r2.length, _ = t(n, 1) >>> 0, a = z(), f = 0;
  for (; f < n; f++) {
    let b = r2.charCodeAt(f);
    if (b > 127) break;
    a[_ + f] = b;
  }
  if (f !== n) {
    f !== 0 && (r2 = r2.slice(f)), _ = e(_, n, n = f + r2.length * 3, 1) >>> 0;
    let b = z().subarray(_ + f, _ + n), g = P.encodeInto(r2, b);
    f += g.written, _ = e(_, n, f, 1) >>> 0;
  }
  return l = f, _;
}
__name(v, "v");
var T = false;
function yt(r2) {
  let t = i.__wbindgen_externrefs.get(r2);
  return i.__externref_table_dealloc(r2), t;
}
__name(yt, "yt");
var it = new TextDecoder("utf-8", { ignoreBOM: true, fatal: true });
it.decode();
function mt(r2, t) {
  return it.decode(z().subarray(r2, r2 + t));
}
__name(mt, "mt");
var P = new TextEncoder();
"encodeInto" in P || (P.encodeInto = function(r2, t) {
  let e = P.encode(r2);
  return t.set(e), { read: r2.length, written: e.length };
});
var l = 0;
var N = new WebAssembly.Instance(Z, rt());
var i = N.exports;
i.__wbindgen_start();
Error.stackTraceLimit = 100;
var p = tt();
function H() {
  p.criticalError && (console.log("Reinitializing Wasm application"), O(), p.criticalError = false, p.instanceId++);
}
__name(H, "H");
addEventListener("error", (r2) => {
  J(r2.error);
});
function J(r2) {
  r2 instanceof WebAssembly.RuntimeError && (console.error("Critical", r2), p.criticalError = true);
}
__name(J, "J");
var M = class extends xt {
  static {
    __name(this, "M");
  }
};
M.prototype.fetch = function(t) {
  return et.call(this, t, this.env, this.ctx);
};
M.prototype.init = nt;
var It = { set: /* @__PURE__ */ __name((r2, t, e, n) => Reflect.set(r2.instance, t, e, n), "set"), has: /* @__PURE__ */ __name((r2, t) => Reflect.has(r2.instance, t), "has"), deleteProperty: /* @__PURE__ */ __name((r2, t) => Reflect.deleteProperty(r2.instance, t), "deleteProperty"), apply: /* @__PURE__ */ __name((r2, t, e) => Reflect.apply(r2.instance, t, e), "apply"), construct: /* @__PURE__ */ __name((r2, t, e) => Reflect.construct(r2.instance, t, e), "construct"), getPrototypeOf: /* @__PURE__ */ __name((r2) => Reflect.getPrototypeOf(r2.instance), "getPrototypeOf"), setPrototypeOf: /* @__PURE__ */ __name((r2, t) => Reflect.setPrototypeOf(r2.instance, t), "setPrototypeOf"), isExtensible: /* @__PURE__ */ __name((r2) => Reflect.isExtensible(r2.instance), "isExtensible"), preventExtensions: /* @__PURE__ */ __name((r2) => Reflect.preventExtensions(r2.instance), "preventExtensions"), getOwnPropertyDescriptor: /* @__PURE__ */ __name((r2, t) => Reflect.getOwnPropertyDescriptor(r2.instance, t), "getOwnPropertyDescriptor"), defineProperty: /* @__PURE__ */ __name((r2, t, e) => Reflect.defineProperty(r2.instance, t, e), "defineProperty"), ownKeys: /* @__PURE__ */ __name((r2) => Reflect.ownKeys(r2.instance), "ownKeys") };
var y = { construct(r2, t, e) {
  try {
    H();
    let n = { instance: Reflect.construct(r2, t, e), instanceId: p.instanceId, ctor: r2, args: t, newTarget: e };
    return new Proxy(n, { ...It, get(_, a, f) {
      _.instanceId !== p.instanceId && (_.instance = Reflect.construct(_.ctor, _.args, _.newTarget), _.instanceId = p.instanceId);
      let b = Reflect.get(_.instance, a, f);
      return typeof b != "function" ? b : b.constructor === Function ? new Proxy(b, { apply(g, U, C) {
        H();
        try {
          return g.apply(U, C);
        } catch (W) {
          throw J(W), W;
        }
      } }) : new Proxy(b, { async apply(g, U, C) {
        H();
        try {
          return await g.apply(U, C);
        } catch (W) {
          throw J(W), W;
        }
      } });
    } });
  } catch (n) {
    throw p.criticalError = true, n;
  }
} };
var jt = new Proxy(M, y);
var Wt = new Proxy(I, y);
var kt = new Proxy(E, y);
var At = new Proxy(R, y);
var zt = new Proxy(F, y);
var Pt = new Proxy(S, y);
var Mt = new Proxy(x, y);
var Ot = new Proxy(j, y);

// ../../../.npm/_npx/32026684e21afda6/node_modules/wrangler/templates/middleware/middleware-ensure-req-body-drained.ts
var drainBody = /* @__PURE__ */ __name(async (request, env, _ctx, middlewareCtx) => {
  try {
    return await middlewareCtx.next(request, env);
  } finally {
    try {
      if (request.body !== null && !request.bodyUsed) {
        const reader = request.body.getReader();
        while (!(await reader.read()).done) {
        }
      }
    } catch (e) {
      console.error("Failed to drain the unused request body.", e);
    }
  }
}, "drainBody");
var middleware_ensure_req_body_drained_default = drainBody;

// ../../../.npm/_npx/32026684e21afda6/node_modules/wrangler/templates/middleware/middleware-miniflare3-json-error.ts
function reduceError(e) {
  return {
    name: e?.name,
    message: e?.message ?? String(e),
    stack: e?.stack,
    cause: e?.cause === void 0 ? void 0 : reduceError(e.cause)
  };
}
__name(reduceError, "reduceError");
var jsonError = /* @__PURE__ */ __name(async (request, env, _ctx, middlewareCtx) => {
  try {
    return await middlewareCtx.next(request, env);
  } catch (e) {
    const error = reduceError(e);
    return Response.json(error, {
      status: 500,
      headers: { "MF-Experimental-Error-Stack": "true" }
    });
  }
}, "jsonError");
var middleware_miniflare3_json_error_default = jsonError;

// .wrangler/tmp/bundle-B4YrQk/middleware-insertion-facade.js
var __INTERNAL_WRANGLER_MIDDLEWARE__ = [
  middleware_ensure_req_body_drained_default,
  middleware_miniflare3_json_error_default
];
var middleware_insertion_facade_default = jt;

// ../../../.npm/_npx/32026684e21afda6/node_modules/wrangler/templates/middleware/common.ts
var __facade_middleware__ = [];
function __facade_register__(...args) {
  __facade_middleware__.push(...args.flat());
}
__name(__facade_register__, "__facade_register__");
function __facade_invokeChain__(request, env, ctx, dispatch, middlewareChain) {
  const [head, ...tail] = middlewareChain;
  const middlewareCtx = {
    dispatch,
    next(newRequest, newEnv) {
      return __facade_invokeChain__(newRequest, newEnv, ctx, dispatch, tail);
    }
  };
  return head(request, env, ctx, middlewareCtx);
}
__name(__facade_invokeChain__, "__facade_invokeChain__");
function __facade_invoke__(request, env, ctx, dispatch, finalMiddleware) {
  return __facade_invokeChain__(request, env, ctx, dispatch, [
    ...__facade_middleware__,
    finalMiddleware
  ]);
}
__name(__facade_invoke__, "__facade_invoke__");

// .wrangler/tmp/bundle-B4YrQk/middleware-loader.entry.ts
var __Facade_ScheduledController__ = class ___Facade_ScheduledController__ {
  constructor(scheduledTime, cron, noRetry) {
    this.scheduledTime = scheduledTime;
    this.cron = cron;
    this.#noRetry = noRetry;
  }
  scheduledTime;
  cron;
  static {
    __name(this, "__Facade_ScheduledController__");
  }
  #noRetry;
  noRetry() {
    if (!(this instanceof ___Facade_ScheduledController__)) {
      throw new TypeError("Illegal invocation");
    }
    this.#noRetry();
  }
};
function wrapExportedHandler(worker) {
  if (__INTERNAL_WRANGLER_MIDDLEWARE__ === void 0 || __INTERNAL_WRANGLER_MIDDLEWARE__.length === 0) {
    return worker;
  }
  for (const middleware of __INTERNAL_WRANGLER_MIDDLEWARE__) {
    __facade_register__(middleware);
  }
  const fetchDispatcher = /* @__PURE__ */ __name(function(request, env, ctx) {
    if (worker.fetch === void 0) {
      throw new Error("Handler does not export a fetch() function.");
    }
    return worker.fetch(request, env, ctx);
  }, "fetchDispatcher");
  return {
    ...worker,
    fetch(request, env, ctx) {
      const dispatcher = /* @__PURE__ */ __name(function(type, init) {
        if (type === "scheduled" && worker.scheduled !== void 0) {
          const controller = new __Facade_ScheduledController__(
            Date.now(),
            init.cron ?? "",
            () => {
            }
          );
          return worker.scheduled(controller, env, ctx);
        }
      }, "dispatcher");
      return __facade_invoke__(request, env, ctx, dispatcher, fetchDispatcher);
    }
  };
}
__name(wrapExportedHandler, "wrapExportedHandler");
function wrapWorkerEntrypoint(klass) {
  if (__INTERNAL_WRANGLER_MIDDLEWARE__ === void 0 || __INTERNAL_WRANGLER_MIDDLEWARE__.length === 0) {
    return klass;
  }
  for (const middleware of __INTERNAL_WRANGLER_MIDDLEWARE__) {
    __facade_register__(middleware);
  }
  return class extends klass {
    #fetchDispatcher = /* @__PURE__ */ __name((request, env, ctx) => {
      this.env = env;
      this.ctx = ctx;
      if (super.fetch === void 0) {
        throw new Error("Entrypoint class does not define a fetch() function.");
      }
      return super.fetch(request);
    }, "#fetchDispatcher");
    #dispatcher = /* @__PURE__ */ __name((type, init) => {
      if (type === "scheduled" && super.scheduled !== void 0) {
        const controller = new __Facade_ScheduledController__(
          Date.now(),
          init.cron ?? "",
          () => {
          }
        );
        return super.scheduled(controller);
      }
    }, "#dispatcher");
    fetch(request) {
      return __facade_invoke__(
        request,
        this.env,
        this.ctx,
        this.#dispatcher,
        this.#fetchDispatcher
      );
    }
  };
}
__name(wrapWorkerEntrypoint, "wrapWorkerEntrypoint");
var WRAPPED_ENTRY;
if (typeof middleware_insertion_facade_default === "object") {
  WRAPPED_ENTRY = wrapExportedHandler(middleware_insertion_facade_default);
} else if (typeof middleware_insertion_facade_default === "function") {
  WRAPPED_ENTRY = wrapWorkerEntrypoint(middleware_insertion_facade_default);
}
var middleware_loader_entry_default = WRAPPED_ENTRY;
export {
  Wt as ContainerStartupOptions,
  kt as Hive,
  At as IntoUnderlyingByteSource,
  zt as IntoUnderlyingSink,
  Pt as IntoUnderlyingSource,
  Mt as MinifyConfig,
  Ot as R2Range,
  __INTERNAL_WRANGLER_MIDDLEWARE__,
  middleware_loader_entry_default as default
};
//# sourceMappingURL=shim.js.map
