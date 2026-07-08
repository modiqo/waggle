var __defProp = Object.defineProperty;
var __name = (target, value) => __defProp(target, "name", { value, configurable: true });

// build/index.js
import { WorkerEntrypoint as xe } from "cloudflare:workers";
import ee from "./29389b2eac672d7946a368bc06bd14e180e4404c-index_bg.wasm";
var G = globalThis.__worker_init_state = { criticalError: false, instanceId: 0 };
var E = class {
  static {
    __name(this, "E");
  }
  __destroy_into_raw() {
    let e = this.__wbg_ptr;
    return this.__wbg_ptr = 0, we.unregister(this), e;
  }
  free() {
    let e = this.__destroy_into_raw();
    c();
    try {
      i.__wbg_containerstartupoptions_free(e, 0);
    } catch (t) {
      s(t);
    }
  }
  get enableInternet() {
    if (this.__wbg_inst !== void 0 && this.__wbg_inst !== o) throw new Error("Invalid stale object from previous Wasm instance");
    let e;
    c();
    try {
      e = i.__wbg_get_containerstartupoptions_enableInternet(this.__wbg_ptr);
    } catch (t) {
      s(t);
    }
    return e === 16777215 ? void 0 : e !== 0;
  }
  get entrypoint() {
    if (this.__wbg_inst !== void 0 && this.__wbg_inst !== o) throw new Error("Invalid stale object from previous Wasm instance");
    let e;
    c();
    try {
      e = i.__wbg_get_containerstartupoptions_entrypoint(this.__wbg_ptr);
    } catch (n) {
      s(n);
    }
    var t = he(e[0], e[1]).slice();
    return i.__wbindgen_free(e[0], e[1] * 4, 4), t;
  }
  get env() {
    if (this.__wbg_inst !== void 0 && this.__wbg_inst !== o) throw new Error("Invalid stale object from previous Wasm instance");
    let e;
    c();
    try {
      e = i.__wbg_get_containerstartupoptions_env(this.__wbg_ptr);
    } catch (t) {
      s(t);
    }
    return e;
  }
  set enableInternet(e) {
    if (this.__wbg_inst !== void 0 && this.__wbg_inst !== o) throw new Error("Invalid stale object from previous Wasm instance");
    c();
    try {
      i.__wbg_set_containerstartupoptions_enableInternet(this.__wbg_ptr, u(e) ? 16777215 : e ? 1 : 0);
    } catch (t) {
      s(t);
    }
  }
  set entrypoint(e) {
    if (this.__wbg_inst !== void 0 && this.__wbg_inst !== o) throw new Error("Invalid stale object from previous Wasm instance");
    let t = pe(e, i.__wbindgen_malloc), n = l;
    c();
    try {
      i.__wbg_set_containerstartupoptions_entrypoint(this.__wbg_ptr, t, n);
    } catch (_) {
      s(_);
    }
  }
  set env(e) {
    if (this.__wbg_inst !== void 0 && this.__wbg_inst !== o) throw new Error("Invalid stale object from previous Wasm instance");
    c();
    try {
      i.__wbg_set_containerstartupoptions_env(this.__wbg_ptr, e);
    } catch (t) {
      s(t);
    }
  }
};
Symbol.dispose && (E.prototype[Symbol.dispose] = E.prototype.free);
var R = class {
  static {
    __name(this, "R");
  }
  __destroy_into_raw() {
    let e = this.__wbg_ptr;
    return this.__wbg_ptr = 0, K.unregister(this), e;
  }
  free() {
    let e = this.__destroy_into_raw();
    c();
    try {
      i.__wbg_hive_free(e, 0);
    } catch (t) {
      s(t);
    }
  }
  alarm() {
    if (this.__wbg_inst !== void 0 && this.__wbg_inst !== o) throw new Error("Invalid stale object from previous Wasm instance");
    let e;
    c();
    try {
      e = i.hive_alarm(this.__wbg_ptr);
    } catch (t) {
      s(t);
    }
    return e;
  }
  fetch(e) {
    if (this.__wbg_inst !== void 0 && this.__wbg_inst !== o) throw new Error("Invalid stale object from previous Wasm instance");
    let t;
    c();
    try {
      t = i.hive_fetch(this.__wbg_ptr, e);
    } catch (n) {
      s(n);
    }
    return t;
  }
  constructor(e, t) {
    let n;
    c();
    try {
      n = i.hive_new(e, t);
    } catch (_) {
      s(_);
    }
    return this.__wbg_ptr = n, Object.defineProperty(this, "__wbg_inst", { value: o, writable: true }), K.register(this, { ptr: n, instance: o }, this), this;
  }
  webSocketClose(e, t, n, _) {
    if (this.__wbg_inst !== void 0 && this.__wbg_inst !== o) throw new Error("Invalid stale object from previous Wasm instance");
    let a = p(n, i.__wbindgen_malloc, i.__wbindgen_realloc), f = l, b;
    c();
    try {
      b = i.hive_webSocketClose(this.__wbg_ptr, e, t, a, f, _);
    } catch (d) {
      s(d);
    }
    return b;
  }
  webSocketError(e, t) {
    if (this.__wbg_inst !== void 0 && this.__wbg_inst !== o) throw new Error("Invalid stale object from previous Wasm instance");
    let n;
    c();
    try {
      n = i.hive_webSocketError(this.__wbg_ptr, e, t);
    } catch (_) {
      s(_);
    }
    return n;
  }
  webSocketMessage(e, t) {
    if (this.__wbg_inst !== void 0 && this.__wbg_inst !== o) throw new Error("Invalid stale object from previous Wasm instance");
    let n;
    c();
    try {
      n = i.hive_webSocketMessage(this.__wbg_ptr, e, t);
    } catch (_) {
      s(_);
    }
    return n;
  }
};
Symbol.dispose && (R.prototype[Symbol.dispose] = R.prototype.free);
var F = class {
  static {
    __name(this, "F");
  }
  __destroy_into_raw() {
    let e = this.__wbg_ptr;
    return this.__wbg_ptr = 0, de.unregister(this), e;
  }
  free() {
    let e = this.__destroy_into_raw();
    c();
    try {
      i.__wbg_intounderlyingbytesource_free(e, 0);
    } catch (t) {
      s(t);
    }
  }
  get autoAllocateChunkSize() {
    if (this.__wbg_inst !== void 0 && this.__wbg_inst !== o) throw new Error("Invalid stale object from previous Wasm instance");
    let e;
    c();
    try {
      e = i.intounderlyingbytesource_autoAllocateChunkSize(this.__wbg_ptr);
    } catch (t) {
      s(t);
    }
    return e >>> 0;
  }
  cancel() {
    if (this.__wbg_inst !== void 0 && this.__wbg_inst !== o) throw new Error("Invalid stale object from previous Wasm instance");
    let e = this.__destroy_into_raw();
    c();
    try {
      i.intounderlyingbytesource_cancel(e);
    } catch (t) {
      s(t);
    }
  }
  pull(e) {
    if (this.__wbg_inst !== void 0 && this.__wbg_inst !== o) throw new Error("Invalid stale object from previous Wasm instance");
    let t;
    c();
    try {
      t = i.intounderlyingbytesource_pull(this.__wbg_ptr, e);
    } catch (n) {
      s(n);
    }
    return t;
  }
  start(e) {
    if (this.__wbg_inst !== void 0 && this.__wbg_inst !== o) throw new Error("Invalid stale object from previous Wasm instance");
    c();
    try {
      i.intounderlyingbytesource_start(this.__wbg_ptr, e);
    } catch (t) {
      s(t);
    }
  }
  get type() {
    if (this.__wbg_inst !== void 0 && this.__wbg_inst !== o) throw new Error("Invalid stale object from previous Wasm instance");
    let e;
    c();
    try {
      e = i.intounderlyingbytesource_type(this.__wbg_ptr);
    } catch (t) {
      s(t);
    }
    return fe[e];
  }
};
Symbol.dispose && (F.prototype[Symbol.dispose] = F.prototype.free);
var S = class {
  static {
    __name(this, "S");
  }
  __destroy_into_raw() {
    let e = this.__wbg_ptr;
    return this.__wbg_ptr = 0, ge.unregister(this), e;
  }
  free() {
    let e = this.__destroy_into_raw();
    c();
    try {
      i.__wbg_intounderlyingsink_free(e, 0);
    } catch (t) {
      s(t);
    }
  }
  abort(e) {
    if (this.__wbg_inst !== void 0 && this.__wbg_inst !== o) throw new Error("Invalid stale object from previous Wasm instance");
    let t = this.__destroy_into_raw(), n;
    c();
    try {
      n = i.intounderlyingsink_abort(t, e);
    } catch (_) {
      s(_);
    }
    return n;
  }
  close() {
    if (this.__wbg_inst !== void 0 && this.__wbg_inst !== o) throw new Error("Invalid stale object from previous Wasm instance");
    let e = this.__destroy_into_raw(), t;
    c();
    try {
      t = i.intounderlyingsink_close(e);
    } catch (n) {
      s(n);
    }
    return t;
  }
  write(e) {
    if (this.__wbg_inst !== void 0 && this.__wbg_inst !== o) throw new Error("Invalid stale object from previous Wasm instance");
    let t;
    c();
    try {
      t = i.intounderlyingsink_write(this.__wbg_ptr, e);
    } catch (n) {
      s(n);
    }
    return t;
  }
};
Symbol.dispose && (S.prototype[Symbol.dispose] = S.prototype.free);
var x = class r {
  static {
    __name(this, "r");
  }
  static __wrap(e) {
    let t = Object.create(r.prototype);
    return t.__wbg_ptr = e, Object.defineProperty(t, "__wbg_inst", { value: o, writable: true }), Q.register(t, { ptr: e, instance: o }, t), t;
  }
  __destroy_into_raw() {
    let e = this.__wbg_ptr;
    return this.__wbg_ptr = 0, Q.unregister(this), e;
  }
  free() {
    let e = this.__destroy_into_raw();
    c();
    try {
      i.__wbg_intounderlyingsource_free(e, 0);
    } catch (t) {
      s(t);
    }
  }
  cancel() {
    if (this.__wbg_inst !== void 0 && this.__wbg_inst !== o) throw new Error("Invalid stale object from previous Wasm instance");
    let e = this.__destroy_into_raw();
    c();
    try {
      i.intounderlyingsource_cancel(e);
    } catch (t) {
      s(t);
    }
  }
  pull(e) {
    if (this.__wbg_inst !== void 0 && this.__wbg_inst !== o) throw new Error("Invalid stale object from previous Wasm instance");
    let t;
    c();
    try {
      t = i.intounderlyingsource_pull(this.__wbg_ptr, e);
    } catch (n) {
      s(n);
    }
    return t;
  }
};
Symbol.dispose && (x.prototype[Symbol.dispose] = x.prototype.free);
var I = class r2 {
  static {
    __name(this, "r");
  }
  static __wrap(e) {
    let t = Object.create(r2.prototype);
    return t.__wbg_ptr = e, Object.defineProperty(t, "__wbg_inst", { value: o, writable: true }), X.register(t, { ptr: e, instance: o }, t), t;
  }
  __destroy_into_raw() {
    let e = this.__wbg_ptr;
    return this.__wbg_ptr = 0, X.unregister(this), e;
  }
  free() {
    let e = this.__destroy_into_raw();
    c();
    try {
      i.__wbg_minifyconfig_free(e, 0);
    } catch (t) {
      s(t);
    }
  }
  get css() {
    if (this.__wbg_inst !== void 0 && this.__wbg_inst !== o) throw new Error("Invalid stale object from previous Wasm instance");
    let e;
    c();
    try {
      e = i.__wbg_get_minifyconfig_css(this.__wbg_ptr);
    } catch (t) {
      s(t);
    }
    return e !== 0;
  }
  get html() {
    if (this.__wbg_inst !== void 0 && this.__wbg_inst !== o) throw new Error("Invalid stale object from previous Wasm instance");
    let e;
    c();
    try {
      e = i.__wbg_get_minifyconfig_html(this.__wbg_ptr);
    } catch (t) {
      s(t);
    }
    return e !== 0;
  }
  get js() {
    if (this.__wbg_inst !== void 0 && this.__wbg_inst !== o) throw new Error("Invalid stale object from previous Wasm instance");
    let e;
    c();
    try {
      e = i.__wbg_get_minifyconfig_js(this.__wbg_ptr);
    } catch (t) {
      s(t);
    }
    return e !== 0;
  }
  set css(e) {
    if (this.__wbg_inst !== void 0 && this.__wbg_inst !== o) throw new Error("Invalid stale object from previous Wasm instance");
    c();
    try {
      i.__wbg_set_minifyconfig_css(this.__wbg_ptr, e);
    } catch (t) {
      s(t);
    }
  }
  set html(e) {
    if (this.__wbg_inst !== void 0 && this.__wbg_inst !== o) throw new Error("Invalid stale object from previous Wasm instance");
    c();
    try {
      i.__wbg_set_minifyconfig_html(this.__wbg_ptr, e);
    } catch (t) {
      s(t);
    }
  }
  set js(e) {
    if (this.__wbg_inst !== void 0 && this.__wbg_inst !== o) throw new Error("Invalid stale object from previous Wasm instance");
    c();
    try {
      i.__wbg_set_minifyconfig_js(this.__wbg_ptr, e);
    } catch (t) {
      s(t);
    }
  }
};
Symbol.dispose && (I.prototype[Symbol.dispose] = I.prototype.free);
var j = class {
  static {
    __name(this, "j");
  }
  __destroy_into_raw() {
    let e = this.__wbg_ptr;
    return this.__wbg_ptr = 0, le.unregister(this), e;
  }
  free() {
    let e = this.__destroy_into_raw();
    c();
    try {
      i.__wbg_r2range_free(e, 0);
    } catch (t) {
      s(t);
    }
  }
  get length() {
    if (this.__wbg_inst !== void 0 && this.__wbg_inst !== o) throw new Error("Invalid stale object from previous Wasm instance");
    let e;
    c();
    try {
      e = i.__wbg_get_r2range_length(this.__wbg_ptr);
    } catch (t) {
      s(t);
    }
    return e[0] === 0 ? void 0 : e[1];
  }
  get offset() {
    if (this.__wbg_inst !== void 0 && this.__wbg_inst !== o) throw new Error("Invalid stale object from previous Wasm instance");
    let e;
    c();
    try {
      e = i.__wbg_get_r2range_offset(this.__wbg_ptr);
    } catch (t) {
      s(t);
    }
    return e[0] === 0 ? void 0 : e[1];
  }
  get suffix() {
    if (this.__wbg_inst !== void 0 && this.__wbg_inst !== o) throw new Error("Invalid stale object from previous Wasm instance");
    let e;
    c();
    try {
      e = i.__wbg_get_r2range_suffix(this.__wbg_ptr);
    } catch (t) {
      s(t);
    }
    return e[0] === 0 ? void 0 : e[1];
  }
  set length(e) {
    if (this.__wbg_inst !== void 0 && this.__wbg_inst !== o) throw new Error("Invalid stale object from previous Wasm instance");
    c();
    try {
      i.__wbg_set_r2range_length(this.__wbg_ptr, !u(e), u(e) ? 0 : e);
    } catch (t) {
      s(t);
    }
  }
  set offset(e) {
    if (this.__wbg_inst !== void 0 && this.__wbg_inst !== o) throw new Error("Invalid stale object from previous Wasm instance");
    c();
    try {
      i.__wbg_set_r2range_offset(this.__wbg_ptr, !u(e), u(e) ? 0 : e);
    } catch (t) {
      s(t);
    }
  }
  set suffix(e) {
    if (this.__wbg_inst !== void 0 && this.__wbg_inst !== o) throw new Error("Invalid stale object from previous Wasm instance");
    c();
    try {
      i.__wbg_set_r2range_suffix(this.__wbg_ptr, !u(e), u(e) ? 0 : e);
    } catch (t) {
      s(t);
    }
  }
};
Symbol.dispose && (j.prototype[Symbol.dispose] = j.prototype.free);
function T() {
  o++, v = null, k = null, A = null, typeof numBytesDecoded < "u" && (numBytesDecoded = 0), typeof l < "u" && (l = 0), V = false, M = false, N = new WebAssembly.Instance(ee, _e()), i = N.exports, i.__wbindgen_start();
}
__name(T, "T");
function te() {
  let r3;
  c();
  try {
    r3 = i.__worker_init_state();
  } catch (e) {
    s(e);
  }
  return r3;
}
__name(te, "te");
function ne(r3, e, t) {
  let n;
  c();
  try {
    n = i.fetch(r3, e, t);
  } catch (_) {
    s(_);
  }
  return n;
}
__name(ne, "ne");
function re() {
  c();
  try {
    i.init();
  } catch (r3) {
    s(r3);
  }
}
__name(re, "re");
function _e() {
  return { __proto__: null, "./index_bg.js": { __proto__: null, __wbg_Error_92b29b0548f8b746: /* @__PURE__ */ __name(function(e, t) {
    return Error(g(e, t));
  }, "__wbg_Error_92b29b0548f8b746"), __wbg_String_8564e559799eccda: /* @__PURE__ */ __name(function(e, t) {
    let n = String(t), _ = p(n, i.__wbindgen_malloc, i.__wbindgen_realloc), a = l;
    w().setInt32(e + 4, a, true), w().setInt32(e + 0, _, true);
  }, "__wbg_String_8564e559799eccda"), __wbg___wbindgen_boolean_get_fa956cfa2d1bd751: /* @__PURE__ */ __name(function(e) {
    let t = e, n = typeof t == "boolean" ? t : void 0;
    return u(n) ? 16777215 : n ? 1 : 0;
  }, "__wbg___wbindgen_boolean_get_fa956cfa2d1bd751"), __wbg___wbindgen_debug_string_c25d447a39f5578f: /* @__PURE__ */ __name(function(e, t) {
    let n = D(t), _ = p(n, i.__wbindgen_malloc, i.__wbindgen_realloc), a = l;
    w().setInt32(e + 4, a, true), w().setInt32(e + 0, _, true);
  }, "__wbg___wbindgen_debug_string_c25d447a39f5578f"), __wbg___wbindgen_in_aca499c5de7ff5e5: /* @__PURE__ */ __name(function(e, t) {
    return e in t;
  }, "__wbg___wbindgen_in_aca499c5de7ff5e5"), __wbg___wbindgen_is_function_1ff95bcc5517c252: /* @__PURE__ */ __name(function(e) {
    return typeof e == "function";
  }, "__wbg___wbindgen_is_function_1ff95bcc5517c252"), __wbg___wbindgen_is_null_ea9085d691f535d3: /* @__PURE__ */ __name(function(e) {
    return e === null;
  }, "__wbg___wbindgen_is_null_ea9085d691f535d3"), __wbg___wbindgen_is_null_or_undefined_f39ff01e68a1554f: /* @__PURE__ */ __name(function(e) {
    return e == null;
  }, "__wbg___wbindgen_is_null_or_undefined_f39ff01e68a1554f"), __wbg___wbindgen_is_object_a27215656b807791: /* @__PURE__ */ __name(function(e) {
    let t = e;
    return typeof t == "object" && t !== null;
  }, "__wbg___wbindgen_is_object_a27215656b807791"), __wbg___wbindgen_is_string_ea5e6cc2e4141dfe: /* @__PURE__ */ __name(function(e) {
    return typeof e == "string";
  }, "__wbg___wbindgen_is_string_ea5e6cc2e4141dfe"), __wbg___wbindgen_is_undefined_c05833b95a3cf397: /* @__PURE__ */ __name(function(e) {
    return e === void 0;
  }, "__wbg___wbindgen_is_undefined_c05833b95a3cf397"), __wbg___wbindgen_jsval_loose_eq_db4c3b15f63fc170: /* @__PURE__ */ __name(function(e, t) {
    return e == t;
  }, "__wbg___wbindgen_jsval_loose_eq_db4c3b15f63fc170"), __wbg___wbindgen_number_get_394265ed1e1b84ee: /* @__PURE__ */ __name(function(e, t) {
    let n = t, _ = typeof n == "number" ? n : void 0;
    w().setFloat64(e + 8, u(_) ? 0 : _, true), w().setInt32(e + 0, !u(_), true);
  }, "__wbg___wbindgen_number_get_394265ed1e1b84ee"), __wbg___wbindgen_reinit_ecd4a8d70c8ce492: /* @__PURE__ */ __name(function() {
    M = true;
  }, "__wbg___wbindgen_reinit_ecd4a8d70c8ce492"), __wbg___wbindgen_string_get_b0ca35b86a603356: /* @__PURE__ */ __name(function(e, t) {
    let n = t, _ = typeof n == "string" ? n : void 0;
    var a = u(_) ? 0 : p(_, i.__wbindgen_malloc, i.__wbindgen_realloc), f = l;
    w().setInt32(e + 4, f, true), w().setInt32(e + 0, a, true);
  }, "__wbg___wbindgen_string_get_b0ca35b86a603356"), __wbg___wbindgen_throw_344f42d3211c4765: /* @__PURE__ */ __name(function(e, t) {
    throw new WebAssembly.Exception(q, [new Error(g(e, t))]);
  }, "__wbg___wbindgen_throw_344f42d3211c4765"), __wbg__wbg_cb_unref_fffb441def202758: /* @__PURE__ */ __name(function(e) {
    e._wbg_cb_unref();
  }, "__wbg__wbg_cb_unref_fffb441def202758"), __wbg_arrayBuffer_c4091d4c830db36d: /* @__PURE__ */ __name(function(e) {
    return e.arrayBuffer();
  }, "__wbg_arrayBuffer_c4091d4c830db36d"), __wbg_body_18c9f2ac15ead4b2: /* @__PURE__ */ __name(function(e) {
    let t = e.body;
    return u(t) ? 0 : h(t);
  }, "__wbg_body_18c9f2ac15ead4b2"), __wbg_buffer_54b87055582c8a81: /* @__PURE__ */ __name(function(e) {
    return e.buffer;
  }, "__wbg_buffer_54b87055582c8a81"), __wbg_byobRequest_06b654bb15590436: /* @__PURE__ */ __name(function(e) {
    let t = e.byobRequest;
    return u(t) ? 0 : h(t);
  }, "__wbg_byobRequest_06b654bb15590436"), __wbg_byteLength_41862ca4020b9c43: /* @__PURE__ */ __name(function(e) {
    return e.byteLength;
  }, "__wbg_byteLength_41862ca4020b9c43"), __wbg_byteOffset_d42e18c4441f628b: /* @__PURE__ */ __name(function(e) {
    return e.byteOffset;
  }, "__wbg_byteOffset_d42e18c4441f628b"), __wbg_call_44b7209e1e252e6a: /* @__PURE__ */ __name(function(e, t, n, _, a) {
    return e.call(t, n, _, a);
  }, "__wbg_call_44b7209e1e252e6a"), __wbg_call_8a2dd23819f8a60a: /* @__PURE__ */ __name(function(e, t) {
    return e.call(t);
  }, "__wbg_call_8a2dd23819f8a60a"), __wbg_call_a6e5c5dce5018821: /* @__PURE__ */ __name(function(e, t, n) {
    return e.call(t, n);
  }, "__wbg_call_a6e5c5dce5018821"), __wbg_call_e3b662382210db98: /* @__PURE__ */ __name(function(e, t, n, _) {
    return e.call(t, n, _);
  }, "__wbg_call_e3b662382210db98"), __wbg_cancel_3983a93e24cc66b3: /* @__PURE__ */ __name(function(e) {
    return e.cancel();
  }, "__wbg_cancel_3983a93e24cc66b3"), __wbg_catch_c1a60df4c30d76d3: /* @__PURE__ */ __name(function(e, t) {
    return e.catch(t);
  }, "__wbg_catch_c1a60df4c30d76d3"), __wbg_cause_5eb2f50e6f22fd6c: /* @__PURE__ */ __name(function(e) {
    return e.cause;
  }, "__wbg_cause_5eb2f50e6f22fd6c"), __wbg_cf_0214917b05eb22e8: /* @__PURE__ */ __name(function(e) {
    let t = e.cf;
    return u(t) ? 0 : h(t);
  }, "__wbg_cf_0214917b05eb22e8"), __wbg_cf_a20029b537f86569: /* @__PURE__ */ __name(function(e) {
    let t = e.cf;
    return u(t) ? 0 : h(t);
  }, "__wbg_cf_a20029b537f86569"), __wbg_close_249a23304523681b: /* @__PURE__ */ __name(function(e) {
    e.close();
  }, "__wbg_close_249a23304523681b"), __wbg_close_72d318d9c16e83ef: /* @__PURE__ */ __name(function(e) {
    e.close();
  }, "__wbg_close_72d318d9c16e83ef"), __wbg_constructor_07a76432cc61e2e7: /* @__PURE__ */ __name(function(e) {
    return e.constructor;
  }, "__wbg_constructor_07a76432cc61e2e7"), __wbg_crypto_38df2bab126b63dc: /* @__PURE__ */ __name(function(e) {
    return e.crypto;
  }, "__wbg_crypto_38df2bab126b63dc"), __wbg_delete_8216f5ee1b1aca49: /* @__PURE__ */ __name(function(e, t, n) {
    return e.delete(g(t, n));
  }, "__wbg_delete_8216f5ee1b1aca49"), __wbg_done_89b2b13e91a60321: /* @__PURE__ */ __name(function(e) {
    return e.done;
  }, "__wbg_done_89b2b13e91a60321"), __wbg_enqueue_6d83b4c6281bafd6: /* @__PURE__ */ __name(function(e, t) {
    e.enqueue(t);
  }, "__wbg_enqueue_6d83b4c6281bafd6"), __wbg_error_744744ff0c9861e6: /* @__PURE__ */ __name(function(e) {
    console.error(e);
  }, "__wbg_error_744744ff0c9861e6"), __wbg_error_7ed559cd7146b49d: /* @__PURE__ */ __name(function(e, t) {
    console.error(e, t);
  }, "__wbg_error_7ed559cd7146b49d"), __wbg_error_a6fa202b58aa1cd3: /* @__PURE__ */ __name(function(e, t) {
    let n, _;
    try {
      n = e, _ = t, console.error(g(e, t));
    } finally {
      c();
      try {
        i.__wbindgen_free(n, _, 1);
      } catch (a) {
        s(a);
      }
    }
  }, "__wbg_error_a6fa202b58aa1cd3"), __wbg_fetch_a730576a617dd701: /* @__PURE__ */ __name(function(e, t) {
    return e.fetch(t);
  }, "__wbg_fetch_a730576a617dd701"), __wbg_getRandomValues_c44a50d8cfdaebeb: /* @__PURE__ */ __name(function(e, t) {
    e.getRandomValues(t);
  }, "__wbg_getRandomValues_c44a50d8cfdaebeb"), __wbg_getReader_bb0230851fbf986b: /* @__PURE__ */ __name(function(e) {
    return e.getReader();
  }, "__wbg_getReader_bb0230851fbf986b"), __wbg_getTime_d6f070c088c9b5ed: /* @__PURE__ */ __name(function(e) {
    return e.getTime();
  }, "__wbg_getTime_d6f070c088c9b5ed"), __wbg_get_07a843c77ca6efba: /* @__PURE__ */ __name(function(e, t, n) {
    return e.get(g(t, n));
  }, "__wbg_get_07a843c77ca6efba"), __wbg_get_18e0163e38e5048d: /* @__PURE__ */ __name(function(e, t, n, _) {
    let a = t.get(g(n, _));
    var f = u(a) ? 0 : p(a, i.__wbindgen_malloc, i.__wbindgen_realloc), b = l;
    w().setInt32(e + 4, b, true), w().setInt32(e + 0, f, true);
  }, "__wbg_get_18e0163e38e5048d"), __wbg_get_395ac2273ef59cb0: /* @__PURE__ */ __name(function(e, t, n, _) {
    let a, f;
    try {
      return a = t, f = n, e.get(g(t, n), _);
    } finally {
      c();
      try {
        i.__wbindgen_free(a, f, 1);
      } catch (b) {
        s(b);
      }
    }
  }, "__wbg_get_395ac2273ef59cb0"), __wbg_get_507a50627bffa49b: /* @__PURE__ */ __name(function(e, t) {
    return e[t >>> 0];
  }, "__wbg_get_507a50627bffa49b"), __wbg_get_78f252d074a84d0b: /* @__PURE__ */ __name(function(e, t) {
    return Reflect.get(e, t);
  }, "__wbg_get_78f252d074a84d0b"), __wbg_get_8f0e18b878d0fab8: /* @__PURE__ */ __name(function(e, t) {
    let n = Reflect.get(e, t);
    return u(n) ? 0 : h(n);
  }, "__wbg_get_8f0e18b878d0fab8"), __wbg_get_be757f0f312c4319: /* @__PURE__ */ __name(function(e, t) {
    return e.get(t);
  }, "__wbg_get_be757f0f312c4319"), __wbg_get_c7eb1f358a7654df: /* @__PURE__ */ __name(function(e, t) {
    return Reflect.get(e, t);
  }, "__wbg_get_c7eb1f358a7654df"), __wbg_get_done_670108eb06ecbe46: /* @__PURE__ */ __name(function(e) {
    let t = e.done;
    return u(t) ? 16777215 : t ? 1 : 0;
  }, "__wbg_get_done_670108eb06ecbe46"), __wbg_get_value_f465f5be30aa0963: /* @__PURE__ */ __name(function(e) {
    return e.value;
  }, "__wbg_get_value_f465f5be30aa0963"), __wbg_headers_7b59c5203c8c475d: /* @__PURE__ */ __name(function(e) {
    return e.headers;
  }, "__wbg_headers_7b59c5203c8c475d"), __wbg_headers_cf9c80f30e2a4eff: /* @__PURE__ */ __name(function(e) {
    return e.headers;
  }, "__wbg_headers_cf9c80f30e2a4eff"), __wbg_idFromName_f87c89f5c31a83a6: /* @__PURE__ */ __name(function(e, t, n) {
    return e.idFromName(g(t, n));
  }, "__wbg_idFromName_f87c89f5c31a83a6"), __wbg_instanceId_27a130234331eb08: /* @__PURE__ */ __name(function(e) {
    return e.instanceId;
  }, "__wbg_instanceId_27a130234331eb08"), __wbg_instanceof_ArrayBuffer_4480b9e0068a8adb: /* @__PURE__ */ __name(function(e) {
    let t;
    try {
      t = e instanceof ArrayBuffer;
    } catch {
      t = false;
    }
    return t;
  }, "__wbg_instanceof_ArrayBuffer_4480b9e0068a8adb"), __wbg_instanceof_Error_1fdac9f13a8181ba: /* @__PURE__ */ __name(function(e) {
    let t;
    try {
      t = e instanceof Error;
    } catch {
      t = false;
    }
    return t;
  }, "__wbg_instanceof_Error_1fdac9f13a8181ba"), __wbg_instanceof_Map_e5b5e3db98422fcc: /* @__PURE__ */ __name(function(e) {
    let t;
    try {
      t = e instanceof Map;
    } catch {
      t = false;
    }
    return t;
  }, "__wbg_instanceof_Map_e5b5e3db98422fcc"), __wbg_instanceof_ReadableStream_3bdc7d10b03fd402: /* @__PURE__ */ __name(function(e) {
    let t;
    try {
      t = e instanceof ReadableStream;
    } catch {
      t = false;
    }
    return t;
  }, "__wbg_instanceof_ReadableStream_3bdc7d10b03fd402"), __wbg_instanceof_Response_c8b64b2256f01bec: /* @__PURE__ */ __name(function(e) {
    let t;
    try {
      t = e instanceof Response;
    } catch {
      t = false;
    }
    return t;
  }, "__wbg_instanceof_Response_c8b64b2256f01bec"), __wbg_instanceof_Uint8Array_309b927aaf7a3fc7: /* @__PURE__ */ __name(function(e) {
    let t;
    try {
      t = e instanceof Uint8Array;
    } catch {
      t = false;
    }
    return t;
  }, "__wbg_instanceof_Uint8Array_309b927aaf7a3fc7"), __wbg_iterator_6f722e4a93058b71: /* @__PURE__ */ __name(function() {
    return Symbol.iterator;
  }, "__wbg_iterator_6f722e4a93058b71"), __wbg_keys_15ebe487809c60dd: /* @__PURE__ */ __name(function(e) {
    return e.keys();
  }, "__wbg_keys_15ebe487809c60dd"), __wbg_length_1f0964f4a5e2c6d8: /* @__PURE__ */ __name(function(e) {
    return e.length;
  }, "__wbg_length_1f0964f4a5e2c6d8"), __wbg_list_301f5869c6f824db: /* @__PURE__ */ __name(function(e, t) {
    return e.list(t);
  }, "__wbg_list_301f5869c6f824db"), __wbg_message_8326fb1d549bebc5: /* @__PURE__ */ __name(function(e) {
    return e.message;
  }, "__wbg_message_8326fb1d549bebc5"), __wbg_method_b84982a2d750a05f: /* @__PURE__ */ __name(function(e, t) {
    let n = t.method, _ = p(n, i.__wbindgen_malloc, i.__wbindgen_realloc), a = l;
    w().setInt32(e + 4, a, true), w().setInt32(e + 0, _, true);
  }, "__wbg_method_b84982a2d750a05f"), __wbg_minifyconfig_new: /* @__PURE__ */ __name(function(e) {
    return I.__wrap(e);
  }, "__wbg_minifyconfig_new"), __wbg_msCrypto_bd5a034af96bcba6: /* @__PURE__ */ __name(function(e) {
    return e.msCrypto;
  }, "__wbg_msCrypto_bd5a034af96bcba6"), __wbg_name_b0b4809690944614: /* @__PURE__ */ __name(function(e) {
    return e.name;
  }, "__wbg_name_b0b4809690944614"), __wbg_name_f4eafd9ffc22cda1: /* @__PURE__ */ __name(function(e) {
    return e.name;
  }, "__wbg_name_f4eafd9ffc22cda1"), __wbg_new_0_3da9e97f24fc69be: /* @__PURE__ */ __name(function() {
    return /* @__PURE__ */ new Date();
  }, "__wbg_new_0_3da9e97f24fc69be"), __wbg_new_0d809930cd1354c6: /* @__PURE__ */ __name(function() {
    return new Headers();
  }, "__wbg_new_0d809930cd1354c6"), __wbg_new_227d7c05414eb861: /* @__PURE__ */ __name(function() {
    return new Error();
  }, "__wbg_new_227d7c05414eb861"), __wbg_new_32b398fb48b6d94a: /* @__PURE__ */ __name(function() {
    return new Array();
  }, "__wbg_new_32b398fb48b6d94a"), __wbg_new_7796ffc7ed656783: /* @__PURE__ */ __name(function() {
    return /* @__PURE__ */ new Map();
  }, "__wbg_new_7796ffc7ed656783"), __wbg_new_83a71d32124ec019: /* @__PURE__ */ __name(function(e) {
    return new FixedLengthStream(e >>> 0);
  }, "__wbg_new_83a71d32124ec019"), __wbg_new_b667d279fd5aa943: /* @__PURE__ */ __name(function(e, t) {
    return new Error(g(e, t));
  }, "__wbg_new_b667d279fd5aa943"), __wbg_new_big_int_76a0d4edcfb9f1f1: /* @__PURE__ */ __name(function(e) {
    return new FixedLengthStream(e);
  }, "__wbg_new_big_int_76a0d4edcfb9f1f1"), __wbg_new_cd45aabdf6073e84: /* @__PURE__ */ __name(function(e) {
    return new Uint8Array(e);
  }, "__wbg_new_cd45aabdf6073e84"), __wbg_new_da52cf8fe3429cb2: /* @__PURE__ */ __name(function() {
    return new Object();
  }, "__wbg_new_da52cf8fe3429cb2"), __wbg_new_typed_1824d93f294193e5: /* @__PURE__ */ __name(function(e, t) {
    try {
      var n = { a: e, b: t }, _ = /* @__PURE__ */ __name((f, b) => {
        let d = n.a;
        n.a = 0;
        try {
          return ae(d, n.b, f, b);
        } finally {
          n.a = d;
        }
      }, "_");
      return new Promise(_);
    } finally {
      n.a = 0;
    }
  }, "__wbg_new_typed_1824d93f294193e5"), __wbg_new_with_byte_offset_and_length_54c7724ee3ec7d82: /* @__PURE__ */ __name(function(e, t, n) {
    return new Uint8Array(e, t >>> 0, n >>> 0);
  }, "__wbg_new_with_byte_offset_and_length_54c7724ee3ec7d82"), __wbg_new_with_into_underlying_source_8812003c9985c511: /* @__PURE__ */ __name(function(e, t) {
    return new ReadableStream(x.__wrap(e), t);
  }, "__wbg_new_with_into_underlying_source_8812003c9985c511"), __wbg_new_with_length_e6785c33c8e4cce8: /* @__PURE__ */ __name(function(e) {
    return new Uint8Array(e >>> 0);
  }, "__wbg_new_with_length_e6785c33c8e4cce8"), __wbg_new_with_opt_buffer_source_and_init_c8d6537f14c8efb5: /* @__PURE__ */ __name(function(e, t) {
    return new Response(e, t);
  }, "__wbg_new_with_opt_buffer_source_and_init_c8d6537f14c8efb5"), __wbg_new_with_opt_readable_stream_and_init_bfd13e2b23ff2e0c: /* @__PURE__ */ __name(function(e, t) {
    return new Response(e, t);
  }, "__wbg_new_with_opt_readable_stream_and_init_bfd13e2b23ff2e0c"), __wbg_new_with_opt_str_and_init_74529c37934bd6b4: /* @__PURE__ */ __name(function(e, t, n) {
    return new Response(e === 0 ? void 0 : g(e, t), n);
  }, "__wbg_new_with_opt_str_and_init_74529c37934bd6b4"), __wbg_new_with_str_and_init_d95cbe11ce28e65e: /* @__PURE__ */ __name(function(e, t, n) {
    return new Request(g(e, t), n);
  }, "__wbg_new_with_str_and_init_d95cbe11ce28e65e"), __wbg_next_6dbf2c0ac8cde20f: /* @__PURE__ */ __name(function(e) {
    return e.next;
  }, "__wbg_next_6dbf2c0ac8cde20f"), __wbg_next_71f2aa1cb3d1e37e: /* @__PURE__ */ __name(function(e) {
    return e.next();
  }, "__wbg_next_71f2aa1cb3d1e37e"), __wbg_node_84ea875411254db1: /* @__PURE__ */ __name(function(e) {
    return e.node;
  }, "__wbg_node_84ea875411254db1"), __wbg_pipeTo_3ecb20e17416edd6: /* @__PURE__ */ __name(function(e, t) {
    return e.pipeTo(t);
  }, "__wbg_pipeTo_3ecb20e17416edd6"), __wbg_process_44c7a14e11e9f69e: /* @__PURE__ */ __name(function(e) {
    return e.process;
  }, "__wbg_process_44c7a14e11e9f69e"), __wbg_prototypesetcall_4770620bbe4688a0: /* @__PURE__ */ __name(function(e, t, n) {
    Uint8Array.prototype.set.call(L(e, t), n);
  }, "__wbg_prototypesetcall_4770620bbe4688a0"), __wbg_put_0cbeed0b6e2ac746: /* @__PURE__ */ __name(function(e, t, n, _) {
    return e.put(g(t, n), _);
  }, "__wbg_put_0cbeed0b6e2ac746"), __wbg_put_7cef41d6e2bc8cd2: /* @__PURE__ */ __name(function(e, t, n, _, a) {
    let f, b;
    try {
      return f = t, b = n, e.put(g(t, n), _, a);
    } finally {
      c();
      try {
        i.__wbindgen_free(f, b, 1);
      } catch (d) {
        s(d);
      }
    }
  }, "__wbg_put_7cef41d6e2bc8cd2"), __wbg_queueMicrotask_0ab5b2d2393e99b9: /* @__PURE__ */ __name(function(e) {
    return e.queueMicrotask;
  }, "__wbg_queueMicrotask_0ab5b2d2393e99b9"), __wbg_queueMicrotask_6a09b7bc46549209: /* @__PURE__ */ __name(function(e) {
    queueMicrotask(e);
  }, "__wbg_queueMicrotask_6a09b7bc46549209"), __wbg_randomFillSync_6c25eac9869eb53c: /* @__PURE__ */ __name(function(e, t) {
    e.randomFillSync(t);
  }, "__wbg_randomFillSync_6c25eac9869eb53c"), __wbg_read_8afa15f12a160ef8: /* @__PURE__ */ __name(function(e) {
    return e.read();
  }, "__wbg_read_8afa15f12a160ef8"), __wbg_readable_e50a8eefee4d38ef: /* @__PURE__ */ __name(function(e) {
    return e.readable;
  }, "__wbg_readable_e50a8eefee4d38ef"), __wbg_releaseLock_5b92874cad775644: /* @__PURE__ */ __name(function(e) {
    e.releaseLock();
  }, "__wbg_releaseLock_5b92874cad775644"), __wbg_require_b4edbdcf3e2a1ef0: /* @__PURE__ */ __name(function() {
    return module.require;
  }, "__wbg_require_b4edbdcf3e2a1ef0"), __wbg_resolve_2191a4dfe481c25b: /* @__PURE__ */ __name(function(e) {
    return Promise.resolve(e);
  }, "__wbg_resolve_2191a4dfe481c25b"), __wbg_respond_510e32df8aeb6817: /* @__PURE__ */ __name(function(e, t) {
    e.respond(t >>> 0);
  }, "__wbg_respond_510e32df8aeb6817"), __wbg_set_0de9c62c23d04ad5: /* @__PURE__ */ __name(function(e, t, n, _, a) {
    e.set(g(t, n), g(_, a));
  }, "__wbg_set_0de9c62c23d04ad5"), __wbg_set_4d7dd76f3dae2926: /* @__PURE__ */ __name(function(e, t, n) {
    e.set(L(t, n));
  }, "__wbg_set_4d7dd76f3dae2926"), __wbg_set_575dd786d51585f8: /* @__PURE__ */ __name(function(e, t, n) {
    return e.set(t, n);
  }, "__wbg_set_575dd786d51585f8"), __wbg_set_6be42768c690e380: /* @__PURE__ */ __name(function(e, t, n) {
    e[t] = n;
  }, "__wbg_set_6be42768c690e380"), __wbg_set_8535240470bf2500: /* @__PURE__ */ __name(function(e, t, n) {
    return Reflect.set(e, t, n);
  }, "__wbg_set_8535240470bf2500"), __wbg_set_8a16b38e4805b298: /* @__PURE__ */ __name(function(e, t, n) {
    e[t >>> 0] = n;
  }, "__wbg_set_8a16b38e4805b298"), __wbg_set_body_029f2d171e0a005f: /* @__PURE__ */ __name(function(e, t) {
    e.body = t;
  }, "__wbg_set_body_029f2d171e0a005f"), __wbg_set_cache_b4a740b195c051f4: /* @__PURE__ */ __name(function(e, t) {
    e.cache = be[t];
  }, "__wbg_set_cache_b4a740b195c051f4"), __wbg_set_criticalError_0d391f2a45c3bc42: /* @__PURE__ */ __name(function(e, t) {
    e.criticalError = t !== 0;
  }, "__wbg_set_criticalError_0d391f2a45c3bc42"), __wbg_set_headers_4be66d6f175ce615: /* @__PURE__ */ __name(function(e, t) {
    e.headers = t;
  }, "__wbg_set_headers_4be66d6f175ce615"), __wbg_set_headers_9c61d123c3ee1f10: /* @__PURE__ */ __name(function(e, t) {
    e.headers = t;
  }, "__wbg_set_headers_9c61d123c3ee1f10"), __wbg_set_high_water_mark_44d043cd607dd13a: /* @__PURE__ */ __name(function(e, t) {
    e.highWaterMark = t;
  }, "__wbg_set_high_water_mark_44d043cd607dd13a"), __wbg_set_instanceId_9b3420954adec865: /* @__PURE__ */ __name(function(e, t) {
    e.instanceId = t >>> 0;
  }, "__wbg_set_instanceId_9b3420954adec865"), __wbg_set_method_5532d59b92d76467: /* @__PURE__ */ __name(function(e, t, n) {
    e.method = g(t, n);
  }, "__wbg_set_method_5532d59b92d76467"), __wbg_set_redirect_badd73a0bcb765e3: /* @__PURE__ */ __name(function(e, t) {
    e.redirect = ue[t];
  }, "__wbg_set_redirect_badd73a0bcb765e3"), __wbg_set_status_3593dfcb55e7ee3c: /* @__PURE__ */ __name(function(e, t) {
    e.status = t;
  }, "__wbg_set_status_3593dfcb55e7ee3c"), __wbg_size_56cb46b36f9b2664: /* @__PURE__ */ __name(function(e) {
    return e.size;
  }, "__wbg_size_56cb46b36f9b2664"), __wbg_stack_3b0d974bbf31e44f: /* @__PURE__ */ __name(function(e, t) {
    let n = t.stack, _ = p(n, i.__wbindgen_malloc, i.__wbindgen_realloc), a = l;
    w().setInt32(e + 4, a, true), w().setInt32(e + 0, _, true);
  }, "__wbg_stack_3b0d974bbf31e44f"), __wbg_static_accessor_GLOBAL_4ef717fb391d88b7: /* @__PURE__ */ __name(function() {
    let e = typeof global > "u" ? null : global;
    return u(e) ? 0 : h(e);
  }, "__wbg_static_accessor_GLOBAL_4ef717fb391d88b7"), __wbg_static_accessor_GLOBAL_THIS_8d1badc68b5a74f4: /* @__PURE__ */ __name(function() {
    let e = typeof globalThis > "u" ? null : globalThis;
    return u(e) ? 0 : h(e);
  }, "__wbg_static_accessor_GLOBAL_THIS_8d1badc68b5a74f4"), __wbg_static_accessor_INIT_STATE_35966a05809b7176: /* @__PURE__ */ __name(function() {
    return G;
  }, "__wbg_static_accessor_INIT_STATE_35966a05809b7176"), __wbg_static_accessor_SELF_146583524fe1469b: /* @__PURE__ */ __name(function() {
    let e = typeof self > "u" ? null : self;
    return u(e) ? 0 : h(e);
  }, "__wbg_static_accessor_SELF_146583524fe1469b"), __wbg_static_accessor_WINDOW_f2829a2234d7819e: /* @__PURE__ */ __name(function() {
    let e = typeof window > "u" ? null : window;
    return u(e) ? 0 : h(e);
  }, "__wbg_static_accessor_WINDOW_f2829a2234d7819e"), __wbg_status_c45b3b9b3033184a: /* @__PURE__ */ __name(function(e) {
    return e.status;
  }, "__wbg_status_c45b3b9b3033184a"), __wbg_storage_63bc3c0e560235be: /* @__PURE__ */ __name(function(e) {
    return e.storage;
  }, "__wbg_storage_63bc3c0e560235be"), __wbg_stringify_b54333f60f1e4dad: /* @__PURE__ */ __name(function(e) {
    return JSON.stringify(e);
  }, "__wbg_stringify_b54333f60f1e4dad"), __wbg_subarray_3ed232c8a6baee09: /* @__PURE__ */ __name(function(e, t, n) {
    return e.subarray(t >>> 0, n >>> 0);
  }, "__wbg_subarray_3ed232c8a6baee09"), __wbg_text_67a40fc1604b6d39: /* @__PURE__ */ __name(function(e) {
    return e.text();
  }, "__wbg_text_67a40fc1604b6d39"), __wbg_then_16d107c451e9905d: /* @__PURE__ */ __name(function(e, t, n) {
    return e.then(t, n);
  }, "__wbg_then_16d107c451e9905d"), __wbg_then_6ec10ae38b3e92f7: /* @__PURE__ */ __name(function(e, t) {
    return e.then(t);
  }, "__wbg_then_6ec10ae38b3e92f7"), __wbg_url_f6cd241d61f89b82: /* @__PURE__ */ __name(function(e, t) {
    let n = t.url, _ = p(n, i.__wbindgen_malloc, i.__wbindgen_realloc), a = l;
    w().setInt32(e + 4, a, true), w().setInt32(e + 0, _, true);
  }, "__wbg_url_f6cd241d61f89b82"), __wbg_value_a5d5488a9589444a: /* @__PURE__ */ __name(function(e) {
    return e.value;
  }, "__wbg_value_a5d5488a9589444a"), __wbg_versions_276b2795b1c6a219: /* @__PURE__ */ __name(function(e) {
    return e.versions;
  }, "__wbg_versions_276b2795b1c6a219"), __wbg_view_21f1d4a4f175dfa9: /* @__PURE__ */ __name(function(e) {
    let t = e.view;
    return u(t) ? 0 : h(t);
  }, "__wbg_view_21f1d4a4f175dfa9"), __wbg_webSocket_d0b2797319952d03: /* @__PURE__ */ __name(function(e) {
    let t = e.webSocket;
    return u(t) ? 0 : h(t);
  }, "__wbg_webSocket_d0b2797319952d03"), __wbg_writable_3b091be22c8b1cf6: /* @__PURE__ */ __name(function(e) {
    return e.writable;
  }, "__wbg_writable_3b091be22c8b1cf6"), __wbindgen_cast_0000000000000001: /* @__PURE__ */ __name(function(e, t) {
    return Z(e, t, se);
  }, "__wbindgen_cast_0000000000000001"), __wbindgen_cast_0000000000000002: /* @__PURE__ */ __name(function(e, t) {
    return Z(e, t, ce);
  }, "__wbindgen_cast_0000000000000002"), __wbindgen_cast_0000000000000003: /* @__PURE__ */ __name(function(e) {
    return e;
  }, "__wbindgen_cast_0000000000000003"), __wbindgen_cast_0000000000000004: /* @__PURE__ */ __name(function(e) {
    return e;
  }, "__wbindgen_cast_0000000000000004"), __wbindgen_cast_0000000000000005: /* @__PURE__ */ __name(function(e, t) {
    return L(e, t);
  }, "__wbindgen_cast_0000000000000005"), __wbindgen_cast_0000000000000006: /* @__PURE__ */ __name(function(e, t) {
    return g(e, t);
  }, "__wbindgen_cast_0000000000000006"), __wbindgen_cast_0000000000000007: /* @__PURE__ */ __name(function(e) {
    return BigInt.asUintN(64, e);
  }, "__wbindgen_cast_0000000000000007"), __wbindgen_init_externref_table: /* @__PURE__ */ __name(function() {
    let e = i.__wbindgen_externrefs, t = e.grow(4);
    e.set(0, void 0), e.set(t + 0, void 0), e.set(t + 1, null), e.set(t + 2, true), e.set(t + 3, false);
  }, "__wbindgen_init_externref_table"), __wbindgen_jstag: WebAssembly.JSTag, __wbindgen_rethrow_critical: /* @__PURE__ */ __name(function(e) {
    throw new Error("Critical error", { cause: e });
  }, "__wbindgen_rethrow_critical") } };
}
__name(_e, "_e");
var q = new WebAssembly.Tag({ parameters: ["externref"] });
var B;
var V = false;
function ie() {
  V = true;
  try {
    let r3 = $()[i.__abort_handler.value / 4];
    r3 && i.__wbindgen_export.get(r3)();
  } catch {
  }
}
__name(ie, "ie");
function s(r3) {
  throw r3 instanceof WebAssembly.Exception && r3.is(q) ? r3.getArg(q, 0) : ($()[B] = 1, ie(), r3);
}
__name(s, "s");
function c() {
  if (B ??= i.__instance_terminated.value / 4, $()[B]) {
    if (V || ie(), M) {
      T();
      return;
    }
    throw new Error("Module terminated");
  } else M && T();
}
__name(c, "c");
function se(r3, e, t) {
  c();
  try {
    i.wasm_bindgen__convert__closures_____invoke__heb53e527852f00e3(r3, e, t);
  } catch (n) {
    s(n);
  }
}
__name(se, "se");
function ce(r3, e, t) {
  let n;
  c();
  try {
    n = i.wasm_bindgen__convert__closures_____invoke__h7a2772d74aff0994(r3, e, t);
  } catch (_) {
    s(_);
  }
  if (n[1]) throw ye(n[0]);
}
__name(ce, "ce");
function ae(r3, e, t, n) {
  c();
  try {
    i.wasm_bindgen__convert__closures_____invoke__h46f0ab9679300f14(r3, e, t, n);
  } catch (_) {
    s(_);
  }
}
__name(ae, "ae");
var fe = ["bytes"];
var be = ["default", "no-store", "reload", "no-cache", "force-cache", "only-if-cached"];
var ue = ["follow", "error", "manual"];
var o = 0;
var we = typeof FinalizationRegistry > "u" ? { register: /* @__PURE__ */ __name(() => {
}, "register"), unregister: /* @__PURE__ */ __name(() => {
}, "unregister") } : new FinalizationRegistry(({ ptr: r3, instance: e }) => {
  e === o && i.__wbg_containerstartupoptions_free(r3, 1);
});
var K = typeof FinalizationRegistry > "u" ? { register: /* @__PURE__ */ __name(() => {
}, "register"), unregister: /* @__PURE__ */ __name(() => {
}, "unregister") } : new FinalizationRegistry(({ ptr: r3, instance: e }) => {
  e === o && i.__wbg_hive_free(r3, 1);
});
var de = typeof FinalizationRegistry > "u" ? { register: /* @__PURE__ */ __name(() => {
}, "register"), unregister: /* @__PURE__ */ __name(() => {
}, "unregister") } : new FinalizationRegistry(({ ptr: r3, instance: e }) => {
  e === o && i.__wbg_intounderlyingbytesource_free(r3, 1);
});
var ge = typeof FinalizationRegistry > "u" ? { register: /* @__PURE__ */ __name(() => {
}, "register"), unregister: /* @__PURE__ */ __name(() => {
}, "unregister") } : new FinalizationRegistry(({ ptr: r3, instance: e }) => {
  e === o && i.__wbg_intounderlyingsink_free(r3, 1);
});
var Q = typeof FinalizationRegistry > "u" ? { register: /* @__PURE__ */ __name(() => {
}, "register"), unregister: /* @__PURE__ */ __name(() => {
}, "unregister") } : new FinalizationRegistry(({ ptr: r3, instance: e }) => {
  e === o && i.__wbg_intounderlyingsource_free(r3, 1);
});
var X = typeof FinalizationRegistry > "u" ? { register: /* @__PURE__ */ __name(() => {
}, "register"), unregister: /* @__PURE__ */ __name(() => {
}, "unregister") } : new FinalizationRegistry(({ ptr: r3, instance: e }) => {
  e === o && i.__wbg_minifyconfig_free(r3, 1);
});
var le = typeof FinalizationRegistry > "u" ? { register: /* @__PURE__ */ __name(() => {
}, "register"), unregister: /* @__PURE__ */ __name(() => {
}, "unregister") } : new FinalizationRegistry(({ ptr: r3, instance: e }) => {
  e === o && i.__wbg_r2range_free(r3, 1);
});
function h(r3) {
  let e = i.__externref_table_alloc();
  return i.__wbindgen_externrefs.set(e, r3), e;
}
__name(h, "h");
var Y = typeof FinalizationRegistry > "u" ? { register: /* @__PURE__ */ __name(() => {
}, "register"), unregister: /* @__PURE__ */ __name(() => {
}, "unregister") } : new FinalizationRegistry((r3) => {
  r3.instance === o && i.__wbindgen_destroy_closure(r3.a, r3.b);
});
function D(r3) {
  let e = typeof r3;
  if (e == "number" || e == "boolean" || r3 == null) return `${r3}`;
  if (e == "string") return `"${r3}"`;
  if (e == "symbol") {
    let _ = r3.description;
    return _ == null ? "Symbol" : `Symbol(${_})`;
  }
  if (e == "function") {
    let _ = r3.name;
    return typeof _ == "string" && _.length > 0 ? `Function(${_})` : "Function";
  }
  if (Array.isArray(r3)) {
    let _ = r3.length, a = "[";
    _ > 0 && (a += D(r3[0]));
    for (let f = 1; f < _; f++) a += ", " + D(r3[f]);
    return a += "]", a;
  }
  let t = /\[object ([^\]]+)\]/.exec(toString.call(r3)), n;
  if (t && t.length > 1) n = t[1];
  else return toString.call(r3);
  if (n == "Object") try {
    return "Object(" + JSON.stringify(r3) + ")";
  } catch {
    return "Object";
  }
  return r3 instanceof Error ? `${r3.name}: ${r3.message}
${r3.stack}` : n;
}
__name(D, "D");
function he(r3, e) {
  r3 = r3 >>> 0;
  let t = w(), n = [];
  for (let _ = r3; _ < r3 + 4 * e; _ += 4) n.push(i.__wbindgen_externrefs.get(t.getUint32(_, true)));
  return i.__externref_drop_slice(r3, e), n;
}
__name(he, "he");
function L(r3, e) {
  return r3 = r3 >>> 0, z().subarray(r3 / 1, r3 / 1 + e);
}
__name(L, "L");
var v = null;
function w() {
  return (v === null || v.buffer.detached === true || v.buffer.detached === void 0 && v.buffer !== i.memory.buffer) && (v = new DataView(i.memory.buffer)), v;
}
__name(w, "w");
var k = null;
function $() {
  return (k === null || k.byteLength === 0) && (k = new Int32Array(i.memory.buffer)), k;
}
__name($, "$");
function g(r3, e) {
  return me(r3 >>> 0, e);
}
__name(g, "g");
var A = null;
function z() {
  return (A === null || A.byteLength === 0) && (A = new Uint8Array(i.memory.buffer)), A;
}
__name(z, "z");
function u(r3) {
  return r3 == null;
}
__name(u, "u");
function Z(r3, e, t) {
  let n = { a: r3, b: e, cnt: 1, instance: o }, _ = /* @__PURE__ */ __name((...a) => {
    if (n.instance !== o) throw new Error("Cannot invoke closure from previous WASM instance");
    n.cnt++;
    let f = n.a;
    n.a = 0;
    try {
      return t(f, n.b, ...a);
    } finally {
      n.a = f, _._wbg_cb_unref();
    }
  }, "_");
  return _._wbg_cb_unref = () => {
    --n.cnt === 0 && (i.__wbindgen_destroy_closure(n.a, n.b), n.a = 0, Y.unregister(n));
  }, Y.register(_, n, n), _;
}
__name(Z, "Z");
function pe(r3, e) {
  let t = e(r3.length * 4, 4) >>> 0;
  for (let n = 0; n < r3.length; n++) {
    let _ = h(r3[n]);
    w().setUint32(t + 4 * n, _, true);
  }
  return l = r3.length, t;
}
__name(pe, "pe");
function p(r3, e, t) {
  if (t === void 0) {
    let b = O.encode(r3), d = e(b.length, 1) >>> 0;
    return z().subarray(d, d + b.length).set(b), l = b.length, d;
  }
  let n = r3.length, _ = e(n, 1) >>> 0, a = z(), f = 0;
  for (; f < n; f++) {
    let b = r3.charCodeAt(f);
    if (b > 127) break;
    a[_ + f] = b;
  }
  if (f !== n) {
    f !== 0 && (r3 = r3.slice(f)), _ = t(_, n, n = f + r3.length * 3, 1) >>> 0;
    let b = z().subarray(_ + f, _ + n), d = O.encodeInto(r3, b);
    f += d.written, _ = t(_, n, f, 1) >>> 0;
  }
  return l = f, _;
}
__name(p, "p");
var M = false;
function ye(r3) {
  let e = i.__wbindgen_externrefs.get(r3);
  return i.__externref_table_dealloc(r3), e;
}
__name(ye, "ye");
var oe = new TextDecoder("utf-8", { ignoreBOM: true, fatal: true });
oe.decode();
function me(r3, e) {
  return oe.decode(z().subarray(r3, r3 + e));
}
__name(me, "me");
var O = new TextEncoder();
"encodeInto" in O || (O.encodeInto = function(r3, e) {
  let t = O.encode(r3);
  return e.set(t), { read: r3.length, written: t.length };
});
var l = 0;
var N = new WebAssembly.Instance(ee, _e());
var i = N.exports;
i.__wbindgen_start();
Error.stackTraceLimit = 100;
var y = te();
function H() {
  y.criticalError && (console.log("Reinitializing Wasm application"), T(), y.criticalError = false, y.instanceId++);
}
__name(H, "H");
addEventListener("error", (r3) => {
  J(r3.error);
});
function J(r3) {
  r3 instanceof WebAssembly.RuntimeError && (console.error("Critical", r3), y.criticalError = true);
}
__name(J, "J");
var P = class extends xe {
  static {
    __name(this, "P");
  }
};
P.prototype.fetch = function(e) {
  return ne.call(this, e, this.env, this.ctx);
};
P.prototype.init = re;
var Ie = { set: /* @__PURE__ */ __name((r3, e, t, n) => Reflect.set(r3.instance, e, t, n), "set"), has: /* @__PURE__ */ __name((r3, e) => Reflect.has(r3.instance, e), "has"), deleteProperty: /* @__PURE__ */ __name((r3, e) => Reflect.deleteProperty(r3.instance, e), "deleteProperty"), apply: /* @__PURE__ */ __name((r3, e, t) => Reflect.apply(r3.instance, e, t), "apply"), construct: /* @__PURE__ */ __name((r3, e, t) => Reflect.construct(r3.instance, e, t), "construct"), getPrototypeOf: /* @__PURE__ */ __name((r3) => Reflect.getPrototypeOf(r3.instance), "getPrototypeOf"), setPrototypeOf: /* @__PURE__ */ __name((r3, e) => Reflect.setPrototypeOf(r3.instance, e), "setPrototypeOf"), isExtensible: /* @__PURE__ */ __name((r3) => Reflect.isExtensible(r3.instance), "isExtensible"), preventExtensions: /* @__PURE__ */ __name((r3) => Reflect.preventExtensions(r3.instance), "preventExtensions"), getOwnPropertyDescriptor: /* @__PURE__ */ __name((r3, e) => Reflect.getOwnPropertyDescriptor(r3.instance, e), "getOwnPropertyDescriptor"), defineProperty: /* @__PURE__ */ __name((r3, e, t) => Reflect.defineProperty(r3.instance, e, t), "defineProperty"), ownKeys: /* @__PURE__ */ __name((r3) => Reflect.ownKeys(r3.instance), "ownKeys") };
var m = { construct(r3, e, t) {
  try {
    H();
    let n = { instance: Reflect.construct(r3, e, t), instanceId: y.instanceId, ctor: r3, args: e, newTarget: t };
    return new Proxy(n, { ...Ie, get(_, a, f) {
      _.instanceId !== y.instanceId && (_.instance = Reflect.construct(_.ctor, _.args, _.newTarget), _.instanceId = y.instanceId);
      let b = Reflect.get(_.instance, a, f);
      return typeof b != "function" ? b : b.constructor === Function ? new Proxy(b, { apply(d, U, C) {
        H();
        try {
          return d.apply(U, C);
        } catch (W) {
          throw J(W), W;
        }
      } }) : new Proxy(b, { async apply(d, U, C) {
        H();
        try {
          return await d.apply(U, C);
        } catch (W) {
          throw J(W), W;
        }
      } });
    } });
  } catch (n) {
    throw y.criticalError = true, n;
  }
} };
var je = new Proxy(P, m);
var We = new Proxy(E, m);
var ke = new Proxy(R, m);
var Ae = new Proxy(F, m);
var ze = new Proxy(S, m);
var Oe = new Proxy(x, m);
var Pe = new Proxy(I, m);
var Te = new Proxy(j, m);

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

// .wrangler/tmp/bundle-MpPwaC/middleware-insertion-facade.js
var __INTERNAL_WRANGLER_MIDDLEWARE__ = [
  middleware_ensure_req_body_drained_default,
  middleware_miniflare3_json_error_default
];
var middleware_insertion_facade_default = je;

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

// .wrangler/tmp/bundle-MpPwaC/middleware-loader.entry.ts
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
  We as ContainerStartupOptions,
  ke as Hive,
  Ae as IntoUnderlyingByteSource,
  ze as IntoUnderlyingSink,
  Oe as IntoUnderlyingSource,
  Pe as MinifyConfig,
  Te as R2Range,
  __INTERNAL_WRANGLER_MIDDLEWARE__,
  middleware_loader_entry_default as default
};
//# sourceMappingURL=shim.js.map
