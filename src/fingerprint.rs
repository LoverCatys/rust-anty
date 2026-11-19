use crate::hardware::{web_device_memory, webgpu_vendor_arch};
use crate::models::Profile;

pub fn build_stealth_script(profile: &Profile) -> String {
    let cpu_threads = profile.cpu.clamp(2, 64);
    let device_mem = web_device_memory(profile.ram);
    let gpu_vendor_json = serde_json::to_string(&profile.gpu_vendor)
        .unwrap_or_else(|_| "\"Google Inc. (NVIDIA)\"".to_string());
    let gpu_renderer_json = serde_json::to_string(&profile.gpu_renderer)
        .unwrap_or_else(|_| {
            "\"ANGLE (NVIDIA, NVIDIA GeForce RTX 3060 (0x00002504) Direct3D11 vs_5_0 ps_5_0, D3D11)\""
                .to_string()
        });
    let (wgpu_vendor, wgpu_arch) = webgpu_vendor_arch(&profile.gpu_vendor, &profile.gpu_name);
    let wgpu_vendor_json = serde_json::to_string(wgpu_vendor).unwrap_or_else(|_| "\"nvidia\"".into());
    let wgpu_arch_json = serde_json::to_string(wgpu_arch).unwrap_or_else(|_| "\"ampere\"".into());

    let width = profile.width.max(800);
    let height = profile.height.max(600);
    let avail_height = height.saturating_sub(40);
    let seed = (profile.noise_seed & 0xFFFF_FFFF) as u32;
    let guard_token = format!(
        "__g_{:016x}{:016x}__",
        rand::random::<u64>(),
        rand::random::<u64>()
    );

    format!(
        r#"(() => {{
    const GUARD_TOKEN = "{guard_token}";
    const SEED = {seed} >>> 0;
    const CPU_THREADS = {cpu_threads};
    const DEVICE_MEMORY = {device_mem};
    const SCREEN_W = {width};
    const SCREEN_H = {height};
    const AVAIL_H = {avail_height};
    const GPU_VENDOR = {gpu_vendor_json};
    const GPU_RENDERER = {gpu_renderer_json};
    const WGPU_VENDOR = {wgpu_vendor_json};
    const WGPU_ARCH = {wgpu_arch_json};

    const patchedRealms = new WeakSet();
    const nativeStrings = new WeakMap();
    const processedAudioBuffers = new WeakSet();
    const drawnCanvases = new WeakSet();
    const mqlMediaOverrides = new WeakMap();

    function mulberry32(a) {{
        return function() {{
            let t = (a += 0x6D2B79F5) >>> 0;
            t = Math.imul(t ^ (t >>> 15), t | 1);
            t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
            return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
        }};
    }}

    function sanitizeNativeThrow(err) {{
        if (err && typeof err === 'object' && typeof err.stack === 'string') {{
            try {{
                const lines = err.stack.split('\n');
                const out = [lines[0]];
                let normalizedTop = false;
                for (let i = 1; i < lines.length; i++) {{
                    const l = lines[i];
                    if (
                        l.includes('invokeNative') ||
                        l.includes('invokeHandler') ||
                        l.includes('sanitizeNativeThrow') ||
                        l.includes('patchRealm')
                    ) {{
                        continue;
                    }}
                    if (!normalizedTop && l.includes('(<anonymous>:')) {{
                        out.push(l.replace(/\(<anonymous>:\d+:\d+\)/, '(<anonymous>)'));
                        normalizedTop = true;
                        continue;
                    }}
                    out.push(l);
                }}
                err.stack = out.join('\n');
            }} catch (_) {{}}
        }}
        throw err;
    }}

    function invokeNative(ReflectObj, origFn, thisArg, args) {{
        try {{
            return ReflectObj.apply(origFn, thisArg, args);
        }} catch (err) {{
            sanitizeNativeThrow(err);
        }}
    }}

    function applySeededImageNoise(imgData) {{
        const d = imgData.data;
        const len = d.length;
        if (len < 64) return imgData;
        const rng = mulberry32((SEED ^ len ^ (imgData.width << 8) ^ imgData.height) >>> 0);
        let modified = 0;
        const stride = Math.max(8, Math.floor(len / 256)) * 4;
        for (let i = 4; i < len - 8 && modified < 6; i += stride) {{
            // Only nudge genuine anti-aliased edge pixels (partially transparent or smooth gradient)
            const a = d[i + 3];
            if ((a > 8 && a < 248) || (d[i] !== d[i + 4] && Math.abs(d[i] - d[i + 4]) < 48)) {{
                const delta = rng() > 0.5 ? 1 : -1;
                const v = d[i + 2] + delta;
                d[i + 2] = v < 0 ? 0 : (v > 255 ? 255 : v);
                modified++;
            }}
        }}
        return imgData;
    }}

    function patchRealm(G) {{
        if (!G || (typeof G !== 'object' && typeof G !== 'function')) return;
        try {{
            if (patchedRealms.has(G)) return;
            patchedRealms.add(G);
        }} catch (_) {{
            return;
        }}

        const Obj = G.Object;
        const Fn = G.Function;
        const Ref = G.Reflect;
        if (!Obj || !Fn || !Ref) return;

        try {{
            if (Fn.prototype.toString.call(Fn.prototype.toString, GUARD_TOKEN) === true) {{
                return;
            }}
        }} catch (_) {{}}

        const origToString = Fn.prototype.toString;
        const customToString = ({{
            toString() {{
                if (arguments.length === 1 && arguments[0] === GUARD_TOKEN) {{
                    return true;
                }}
                if (nativeStrings.has(this)) {{
                    return nativeStrings.get(this);
                }}
                return invokeNative(Ref, origToString, this, []);
            }}
        }}).toString;
        nativeStrings.set(customToString, 'function toString() {{ [native code] }}');
        Obj.defineProperty(Fn.prototype, 'toString', {{
            value: customToString,
            writable: true,
            configurable: true,
            enumerable: false
        }});

        function wrapMethod(proto, methodName, handler) {{
            if (!proto) return;
            const desc = Obj.getOwnPropertyDescriptor(proto, methodName);
            if (!desc || typeof desc.value !== 'function') return;
            const origFn = desc.value;
            const len = origFn.length;
            let wrapped;
            if (len === 0) {{
                wrapped = ({{ [methodName]() {{ return handler(origFn, this, arguments); }} }})[methodName];
            }} else if (len === 1) {{
                wrapped = ({{ [methodName](a) {{ return handler(origFn, this, arguments); }} }})[methodName];
            }} else if (len === 2) {{
                wrapped = ({{ [methodName](a, b) {{ return handler(origFn, this, arguments); }} }})[methodName];
            }} else if (len === 3) {{
                wrapped = ({{ [methodName](a, b, c) {{ return handler(origFn, this, arguments); }} }})[methodName];
            }} else if (len === 4) {{
                wrapped = ({{ [methodName](a, b, c, d) {{ return handler(origFn, this, arguments); }} }})[methodName];
            }} else if (len === 5) {{
                wrapped = ({{ [methodName](a, b, c, d, e) {{ return handler(origFn, this, arguments); }} }})[methodName];
            }} else if (len === 6) {{
                wrapped = ({{ [methodName](a, b, c, d, e, f) {{ return handler(origFn, this, arguments); }} }})[methodName];
            }} else {{
                wrapped = ({{ [methodName](a, b, c, d, e, f, g) {{ return handler(origFn, this, arguments); }} }})[methodName];
            }}
            nativeStrings.set(wrapped, `function ${{methodName}}() {{ [native code] }}`);
            Obj.defineProperty(proto, methodName, {{
                value: wrapped,
                writable: desc.writable,
                enumerable: desc.enumerable,
                configurable: desc.configurable
            }});
        }}

        function wrapGetter(proto, propName, computeVal) {{
            if (!proto) return;
            const desc = Obj.getOwnPropertyDescriptor(proto, propName);
            if (!desc || typeof desc.get !== 'function') return;
            const origGet = desc.get;
            const holder = {{
                get [propName]() {{
                    // Validate `this` brand via native C++ getter first (throws genuine Illegal invocation on empty object)
                    const origVal = invokeNative(Ref, origGet, this, []);
                    return computeVal(origVal, this);
                }}
            }};
            const getter = Obj.getOwnPropertyDescriptor(holder, propName).get;
            nativeStrings.set(getter, `function get ${{propName}}() {{ [native code] }}`);
            Obj.defineProperty(proto, propName, {{
                get: getter,
                set: desc.set,
                enumerable: desc.enumerable,
                configurable: desc.configurable
            }});
        }}

        // 1. Navigator & WorkerNavigator spoofing (both Window and Worker!)
        //    Only hooks that actually change a value are installed: every wrapper is
        //    potential detection surface, so `platform` / UA-CH (already native Win32/x86/64
        //    on a Windows x64 host) are left untouched, and `hardwareConcurrency` is only
        //    wrapped when it differs from the real core count.
        const navProtos = [
            G.Navigator && G.Navigator.prototype,
            G.WorkerNavigator && G.WorkerNavigator.prototype
        ];
        let realHc, realDm;
        try {{ realHc = G.navigator && G.navigator.hardwareConcurrency; }} catch (_) {{}}
        try {{ realDm = G.navigator && G.navigator.deviceMemory; }} catch (_) {{}}
        for (const np of navProtos) {{
            if (!np) continue;
            if (realHc !== CPU_THREADS) wrapGetter(np, 'hardwareConcurrency', () => CPU_THREADS);
            if (realDm !== DEVICE_MEMORY) wrapGetter(np, 'deviceMemory', () => DEVICE_MEMORY);
        }}

        // 2. Screen & Viewport consistency (Window only).
        //    Skipped entirely when the real monitor already matches the profile:
        //    then screen / outer* / matchMedia stay 100% native (zero hook surface).
        let spoofScreen = false;
        try {{
            spoofScreen = !!(G.screen && (G.screen.width !== SCREEN_W || G.screen.height !== SCREEN_H));
        }} catch (_) {{}}
        if (spoofScreen && G.Screen && G.Screen.prototype) {{
            const sp = G.Screen.prototype;
            wrapGetter(sp, 'width', () => SCREEN_W);
            wrapGetter(sp, 'height', () => SCREEN_H);
            wrapGetter(sp, 'availWidth', () => SCREEN_W);
            wrapGetter(sp, 'availHeight', () => AVAIL_H);
            wrapGetter(sp, 'availTop', () => 0);
            wrapGetter(sp, 'availLeft', () => 0);
            wrapGetter(sp, 'colorDepth', () => 24);
            wrapGetter(sp, 'pixelDepth', () => 24);
            if ('isExtended' in sp) {{
                wrapGetter(sp, 'isExtended', () => false);
            }}
        }}

        // Prevent outerWidth/innerWidth from exceeding spoofed screen dimensions if window is maximized on larger monitor
        if (spoofScreen && 'outerWidth' in G) {{
            wrapGetter(G, 'outerWidth', (orig) => (orig > SCREEN_W ? SCREEN_W : orig));
            wrapGetter(G, 'outerHeight', (orig) => (orig > AVAIL_H ? AVAIL_H : orig));
            wrapGetter(G, 'innerWidth', (orig) => (orig > SCREEN_W ? SCREEN_W : orig));
            wrapGetter(G, 'innerHeight', (orig) => (orig > AVAIL_H ? AVAIL_H : orig));
        }}

        // Sync CSS `device-width` / `device-height` media queries with spoofed Screen
        if (spoofScreen && typeof G.matchMedia === 'function') {{
            if (G.MediaQueryList && G.MediaQueryList.prototype) {{
                wrapGetter(G.MediaQueryList.prototype, 'media', (orig, self) =>
                    mqlMediaOverrides.has(self) ? mqlMediaOverrides.get(self) : orig
                );
            }}
            wrapMethod(G, 'matchMedia', function invokeHandler(origFn, self, args) {{
                let q = args.length > 0 ? String(args[0]) : '';
                if (q.includes('device-width') || q.includes('device-height')) {{
                    // Evaluate deprecated device-width/height features against SCREEN_W / SCREEN_H
                    const patched = q
                        .replace(/\(\s*device-width\s*:\s*(\d+)px\s*\)/gi, (_, n) =>
                            Number(n) === SCREEN_W ? '(min-width: 0px)' : '(max-width: 0px)')
                        .replace(/\(\s*device-height\s*:\s*(\d+)px\s*\)/gi, (_, n) =>
                            Number(n) === SCREEN_H ? '(min-height: 0px)' : '(max-height: 0px)')
                        .replace(/\(\s*min-device-width\s*:\s*(\d+)px\s*\)/gi, (_, n) =>
                            SCREEN_W >= Number(n) ? '(min-width: 0px)' : '(max-width: 0px)')
                        .replace(/\(\s*max-device-width\s*:\s*(\d+)px\s*\)/gi, (_, n) =>
                            SCREEN_W <= Number(n) ? '(min-width: 0px)' : '(max-width: 0px)')
                        .replace(/\(\s*min-device-height\s*:\s*(\d+)px\s*\)/gi, (_, n) =>
                            SCREEN_H >= Number(n) ? '(min-height: 0px)' : '(max-height: 0px)')
                        .replace(/\(\s*max-device-height\s*:\s*(\d+)px\s*\)/gi, (_, n) =>
                            SCREEN_H <= Number(n) ? '(min-height: 0px)' : '(max-height: 0px)');
                    const mql = invokeNative(Ref, origFn, self, [patched]);
                    if (mql) {{
                        try {{
                            mqlMediaOverrides.set(mql, invokeNative(Ref, origFn, self, args).media);
                        }} catch (_) {{}}
                    }}
                    return mql;
                }}
                return invokeNative(Ref, origFn, self, args);
            }});
        }}

        // 3. WebGL 1 & 2 (both Window HTMLCanvasElement and Worker OffscreenCanvas!)
        // Calling native `origFn` FIRST preserves `null` + `GL_INVALID_ENUM` when `WEBGL_debug_renderer_info` is NOT enabled!
        const glProtos = [
            G.WebGLRenderingContext && G.WebGLRenderingContext.prototype,
            G.WebGL2RenderingContext && G.WebGL2RenderingContext.prototype
        ];
        for (const gp of glProtos) {{
            if (!gp) continue;
            wrapMethod(gp, 'getParameter', function invokeHandler(origFn, self, args) {{
                const res = invokeNative(Ref, origFn, self, args);
                if (res === null || res === undefined) return res;
                const pname = args[0];
                if (pname === 37445) return GPU_VENDOR;
                if (pname === 37446) return GPU_RENDERER;
                return res;
            }});
        }}

        // 4. WebGPU (`GPUAdapterInfo` in both Window and Worker!)
        if (G.GPUAdapterInfo && G.GPUAdapterInfo.prototype) {{
            const gip = G.GPUAdapterInfo.prototype;
            wrapGetter(gip, 'vendor', () => WGPU_VENDOR);
            wrapGetter(gip, 'architecture', () => WGPU_ARCH);
            wrapGetter(gip, 'device', () => '');
            wrapGetter(gip, 'description', () => '');
        }}

        // 5. AudioBuffer deterministic per-profile noise (non-accumulating)
        if (G.AudioBuffer && G.AudioBuffer.prototype) {{
            const abp = G.AudioBuffer.prototype;
            const origGetChannel = Obj.getOwnPropertyDescriptor(abp, 'getChannelData')?.value;
            if (typeof origGetChannel === 'function') {{
                function noiseAudioOnce(buf) {{
                    if (!buf || typeof buf !== 'object' || processedAudioBuffers.has(buf)) return;
                    const chCount = buf.numberOfChannels;
                    if (typeof chCount !== 'number') return;
                    processedAudioBuffers.add(buf);
                    for (let c = 0; c < chCount; c++) {{
                        const arr = invokeNative(Ref, origGetChannel, buf, [c]);
                        const rng = mulberry32((SEED ^ ((c + 1) * 0x9E3779B9) ^ arr.length) >>> 0);
                        for (let i = 0; i < arr.length; i += 64) {{
                            if (arr[i] !== 0) {{
                                arr[i] += (rng() - 0.5) * 1e-7;
                            }}
                        }}
                    }}
                }}

                wrapMethod(abp, 'getChannelData', function invokeHandler(origFn, self, args) {{
                    const res = invokeNative(Ref, origFn, self, args);
                    noiseAudioOnce(self);
                    return res;
                }});

                if ('copyFromChannel' in abp) {{
                    wrapMethod(abp, 'copyFromChannel', function invokeHandler(origFn, self, args) {{
                        noiseAudioOnce(self);
                        return invokeNative(Ref, origFn, self, args);
                    }});
                }}
            }}
        }}

        // 6. Canvas 2D & OffscreenCanvas 2D (only noise canvases that rendered text/curves)
        const ctx2dProtos = [
            G.CanvasRenderingContext2D && G.CanvasRenderingContext2D.prototype,
            G.OffscreenCanvasRenderingContext2D && G.OffscreenCanvasRenderingContext2D.prototype
        ];

        for (const cp of ctx2dProtos) {{
            if (!cp) continue;

            for (const drawOp of ['fillText', 'strokeText', 'arc', 'ellipse', 'bezierCurveTo', 'quadraticCurveTo']) {{
                if (drawOp in cp) {{
                    wrapMethod(cp, drawOp, function invokeHandler(origFn, self, args) {{
                        const out = invokeNative(Ref, origFn, self, args);
                        if (self && self.canvas) drawnCanvases.add(self.canvas);
                        return out;
                    }});
                }}
            }}

            if ('getImageData' in cp) {{
                wrapMethod(cp, 'getImageData', function invokeHandler(origFn, self, args) {{
                    const img = invokeNative(Ref, origFn, self, args);
                    if (self && self.canvas && drawnCanvases.has(self.canvas)) {{
                        applySeededImageNoise(img);
                    }}
                    return img;
                }});
            }}
        }}

        if (G.HTMLCanvasElement && G.HTMLCanvasElement.prototype) {{
            const hcp = G.HTMLCanvasElement.prototype;
            const origGetCtx = hcp.getContext;
            const origGetImg = G.CanvasRenderingContext2D?.prototype?.getImageData;

            function makeNoisedCloneIfDrawn(canvasEl) {{
                if (!canvasEl || !drawnCanvases.has(canvasEl) || !G.document) return null;
                const w = canvasEl.width;
                const h = canvasEl.height;
                if (!w || !h || w > 4096 || h > 4096) return null;
                try {{
                    const ctx = Ref.apply(origGetCtx, canvasEl, ['2d']);
                    if (!ctx || !origGetImg) return null;
                    const clone = G.document.createElement('canvas');
                    clone.width = w;
                    clone.height = h;
                    const cctx = Ref.apply(origGetCtx, clone, ['2d']);
                    // `ctx.getImageData` is already wrapped and applies `applySeededImageNoise` once!
                    const img = ctx.getImageData(0, 0, w, h);
                    cctx.putImageData(img, 0, 0);
                    return clone;
                }} catch (_) {{
                    return null;
                }}
            }}

            wrapMethod(hcp, 'toDataURL', function invokeHandler(origFn, self, args) {{
                // Validate `this` first if not an HTMLCanvasElement
                if (!(self instanceof G.HTMLCanvasElement)) {{
                    return invokeNative(Ref, origFn, self, args);
                }}
                const target = makeNoisedCloneIfDrawn(self) || self;
                return invokeNative(Ref, origFn, target, args);
            }});

            if ('toBlob' in hcp) {{
                wrapMethod(hcp, 'toBlob', function invokeHandler(origFn, self, args) {{
                    if (!(self instanceof G.HTMLCanvasElement)) {{
                        return invokeNative(Ref, origFn, self, args);
                    }}
                    const target = makeNoisedCloneIfDrawn(self) || self;
                    return invokeNative(Ref, origFn, target, args);
                }});
            }}
        }}

        if (G.OffscreenCanvas && G.OffscreenCanvas.prototype && 'convertToBlob' in G.OffscreenCanvas.prototype) {{
            const ocp = G.OffscreenCanvas.prototype;
            wrapMethod(ocp, 'convertToBlob', function invokeHandler(origFn, self, args) {{
                if (!(self instanceof G.OffscreenCanvas) || !drawnCanvases.has(self)) {{
                    return invokeNative(Ref, origFn, self, args);
                }}
                try {{
                    const w = self.width;
                    const h = self.height;
                    const ctx = self.getContext('2d');
                    if (ctx && w > 0 && h > 0 && w <= 4096 && h <= 4096) {{
                        const clone = new G.OffscreenCanvas(w, h);
                        const cctx = clone.getContext('2d');
                        cctx.putImageData(ctx.getImageData(0, 0, w, h), 0, 0);
                        return invokeNative(Ref, origFn, clone, args);
                    }}
                }} catch (_) {{}}
                return invokeNative(Ref, origFn, self, args);
            }});
        }}

        // 7. Synchronous same-origin `<iframe>` & popup realm patching
        //    (defeats `ifr.contentWindow`, `window[0]`, `insertAdjacentHTML`, `document.write`, `window.open`!)
        if (G.HTMLIFrameElement && G.HTMLIFrameElement.prototype) {{
            const ifp = G.HTMLIFrameElement.prototype;
            wrapGetter(ifp, 'contentWindow', (win) => {{
                if (win) {{
                    try {{ patchRealm(win); }} catch (_) {{}}
                }}
                return win;
            }});
            wrapGetter(ifp, 'contentDocument', (doc) => {{
                if (doc && doc.defaultView) {{
                    try {{ patchRealm(doc.defaultView); }} catch (_) {{}}
                }}
                return doc;
            }});
        }}

        if (typeof G.open === 'function') {{
            wrapMethod(G, 'open', function invokeHandler(origFn, self, args) {{
                const win = invokeNative(Ref, origFn, self, args);
                if (win) {{
                    try {{ patchRealm(win); }} catch (_) {{}}
                }}
                return win;
            }});
        }}

        function scanInsertedNode(node) {{
            if (!node || typeof node !== 'object') return;
            try {{
                if (node.tagName === 'IFRAME' && node.contentWindow) {{
                    patchRealm(node.contentWindow);
                }} else if (typeof node.querySelectorAll === 'function') {{
                    const frames = node.querySelectorAll('iframe');
                    for (let i = 0; i < frames.length; i++) {{
                        if (frames[i].contentWindow) patchRealm(frames[i].contentWindow);
                    }}
                }}
            }} catch (_) {{}}
        }}

        if (G.Node && G.Node.prototype) {{
            for (const m of ['appendChild', 'insertBefore', 'replaceChild']) {{
                wrapMethod(G.Node.prototype, m, function invokeHandler(origFn, self, args) {{
                    const res = invokeNative(Ref, origFn, self, args);
                    scanInsertedNode(args[0]);
                    return res;
                }});
            }}
        }}
        if (G.Element && G.Element.prototype) {{
            for (const m of ['append', 'prepend', 'after', 'before', 'replaceWith']) {{
                if (m in G.Element.prototype) {{
                    wrapMethod(G.Element.prototype, m, function invokeHandler(origFn, self, args) {{
                        const res = invokeNative(Ref, origFn, self, args);
                        for (let i = 0; i < args.length; i++) scanInsertedNode(args[i]);
                        return res;
                    }});
                }}
            }}
            if ('insertAdjacentElement' in G.Element.prototype) {{
                wrapMethod(G.Element.prototype, 'insertAdjacentElement', function invokeHandler(origFn, self, args) {{
                    const res = invokeNative(Ref, origFn, self, args);
                    scanInsertedNode(args[1]);
                    return res;
                }});
            }}
            if ('insertAdjacentHTML' in G.Element.prototype) {{
                wrapMethod(G.Element.prototype, 'insertAdjacentHTML', function invokeHandler(origFn, self, args) {{
                    const res = invokeNative(Ref, origFn, self, args);
                    if (String(args[1] || '').toLowerCase().includes('<iframe')) {{
                        scanInsertedNode(self.parentNode || self);
                    }}
                    return res;
                }});
            }}
            for (const htmlProp of ['innerHTML', 'outerHTML']) {{
                const d = Obj.getOwnPropertyDescriptor(G.Element.prototype, htmlProp);
                if (d && d.set) {{
                    const origSet = d.set;
                    const holder = {{
                        set [htmlProp](val) {{
                            const r = invokeNative(Ref, origSet, this, [val]);
                            if (String(val).toLowerCase().includes('<iframe')) {{
                                scanInsertedNode(this);
                            }}
                            return r;
                        }}
                    }};
                    const setter = Obj.getOwnPropertyDescriptor(holder, htmlProp).set;
                    nativeStrings.set(setter, `function set ${{htmlProp}}() {{ [native code] }}`);
                    Obj.defineProperty(G.Element.prototype, htmlProp, {{
                        get: d.get,
                        set: setter,
                        enumerable: d.enumerable,
                        configurable: d.configurable
                    }});
                }}
            }}
        }}
        if (G.Document && G.Document.prototype) {{
            for (const m of ['write', 'writeln']) {{
                if (m in G.Document.prototype) {{
                    wrapMethod(G.Document.prototype, m, function invokeHandler(origFn, self, args) {{
                        const res = invokeNative(Ref, origFn, self, args);
                        for (let i = 0; i < args.length; i++) {{
                            if (String(args[i] || '').toLowerCase().includes('<iframe')) {{
                                scanInsertedNode(self);
                                break;
                            }}
                        }}
                        return res;
                    }});
                }}
            }}
        }}
    }}

        patchRealm(globalThis);
}})();"#,
        guard_token = guard_token,
        seed = seed,
        cpu_threads = cpu_threads,
        device_mem = device_mem,
        width = width,
        height = height,
        avail_height = avail_height,
        gpu_vendor_json = gpu_vendor_json,
        gpu_renderer_json = gpu_renderer_json,
        wgpu_vendor_json = wgpu_vendor_json,
        wgpu_arch_json = wgpu_arch_json,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stealth_script_minimal_hooks() {
        let prof = Profile {
            cpu_name: "AMD Ryzen 5 5600X".into(),
            cpu: 12,
            ram: 32,
            gpu_name: "AMD Radeon RX 6600".into(),
            gpu_vendor: "Google Inc. (AMD)".into(),
            gpu_renderer: "ANGLE (AMD, AMD Radeon RX 6600 (0x000073FF) Direct3D11 vs_5_0 ps_5_0, D3D11)".into(),
            width: 1920,
            height: 1080,
            timezone: "Europe/London".into(),
            noise_seed: 0x87654321,
            fonts: vec!["Arial".into(), "Calibri".into()],
            proxy: None,
            latitude: Some(51.5085),
            longitude: Some(-0.1257),
            folder_id: None,
        };

        let script = build_stealth_script(&prof);
        assert!(script.contains("const CPU_THREADS = 12;"));
        assert!(script.contains("const DEVICE_MEMORY = 8;"));
        assert!(script.contains("const WGPU_VENDOR = \"amd\";"));
        assert!(script.contains("const WGPU_ARCH = \"rdna-2\";"));
        // Fonts and platform must NOT be JS-hooked
        assert!(!script.contains("FontFaceSet"));
        assert!(!script.contains("measureText"));
        assert!(!script.contains("getHighEntropyValues"));
        assert!(!script.contains("webdriver"));
    }
}
