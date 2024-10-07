import * as abortSignal from "ext:deno_web/03_abort_signal.js";
import * as base64 from "ext:deno_web/05_base64.js";
import * as compression from "ext:deno_web/14_compression.js";
import * as domException from "ext:deno_web/01_dom_exception.js";
import * as encoding from "ext:deno_web/08_text_encoding.js";
import * as event from "ext:deno_web/02_event.js";
import * as file from "ext:deno_web/09_file.js";
import * as fileReader from "ext:deno_web/10_filereader.js";
import * as globalInterfaces from "ext:deno_web/04_global_interfaces.js";
import * as imageData from "ext:deno_web/16_image_data.js";
import * as location from "ext:deno_web/12_location.js";
import * as messagePort from "ext:deno_web/13_message_port.js";
import * as performance from "ext:deno_web/15_performance.js";
import * as streams from "ext:deno_web/06_streams.js";
import * as timers from "ext:deno_web/02_timers.js";
import * as url from "ext:deno_url/00_url.js";
import * as urlPattern from "ext:deno_url/01_urlpattern.js";

import { core } from "ext:core/mod.js";

globalThis.AbortController = abortSignal.AbortController;
globalThis.AbortSignal = abortSignal.AbortSignal;
globalThis.Blob = file.Blob;
globalThis.CloseEvent = event.CloseEvent;
globalThis.CompressionStream = compression.CompressionStream;
globalThis.CountQueuingStrategy = streams.CountQueuingStrategy;
globalThis.CustomEvent = event.CustomEvent;
globalThis.DOMException = domException.DOMException;
globalThis.ErrorEvent = event.ErrorEvent;
globalThis.Event = event.Event;
globalThis.EventTarget = event.EventTarget;
globalThis.File = file.File;
globalThis.FileReader = fileReader.FileReader;
globalThis.ImageData = imageData.ImageData;
globalThis.MessageChannel = messagePort.MessageChannel;
globalThis.MessageEvent = event.MessageEvent;
globalThis.MessagePort = messagePort.MessagePort;
globalThis.Performance = performance.Performance;
globalThis.PerformanceEntry = performance.PerformanceEntry;
globalThis.PerformanceMark = performance.PerformanceMark;
globalThis.PerformanceMeasure = performance.PerformanceMeasure;
globalThis.PromiseRejectionEvent = event.PromiseRejectionEvent;
globalThis.ReadableByteStreamController = streams.ReadableByteStreamController;
globalThis.ReadableStream = streams.ReadableStream;
globalThis.ReadableStreamBYOBReader = streams.ReadableStreamBYOBReader;
globalThis.ReadableStreamBYOBRequest = streams.ReadableStreamBYOBRequest;
globalThis.ReadableStreamDefaultController = streams.ReadableStreamDefaultController;
globalThis.ReadableStreamDefaultReader = streams.ReadableStreamDefaultReader;
globalThis.TextDecoder = encoding.TextDecoder;
globalThis.TextDecoderStream = encoding.TextDecoderStream;
globalThis.TextEncoder = encoding.TextEncoder;
globalThis.TextEncoderStream = encoding.TextEncoderStream;
globalThis.TransformStream = streams.TransformStream;
globalThis.URL = url.URL;
globalThis.URLPattern = urlPattern.URLPattern;
globalThis.URLSearchParams = url.URLSearchParams;
globalThis.WritableStream = streams.WritableStream;
globalThis.WritableStreamDefaultController = streams.WritableStreamDefaultController;
globalThis.WritableStreamDefaultWriter = streams.WritableStreamDefaultWriter;
globalThis.atob = base64.atob;
globalThis.btoa = base64.btoa;
globalThis.clearInterval = timers.clearInterval;
globalThis.clearTimeout = timers.clearTimeout;
globalThis.performance = performance.performance;
globalThis.setInterval = timers.setInterval;
globalThis.setTimeout = timers.setTimeout;
globalThis.structuredClone = messagePort.structuredClone;

Object.defineProperty(globalThis, 'location', location.workerLocationDescriptor);
Object.defineProperty(globalThis, 'WorkerGlobalScope', globalInterfaces.workerGlobalScopeConstructorDescriptor);
Object.defineProperty(globalThis, 'DedicatedWorkerGlobalScope', globalInterfaces.dedicatedWorkerGlobalScopeConstructorDescriptor);

globalThis.self = globalThis;

location.setLocationHref('https://cohost.org/');

globalThis.process = {
    env: {
        NODE_ENV: "production",
    },
    cwd() {
        return '/';
    }
};
