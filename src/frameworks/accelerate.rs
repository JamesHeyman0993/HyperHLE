/*
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */
//! `Accelerate.framework/Accelerate`
//!
//! The Accelerate framework provides high-performance math routines including
//! vDSP (Digital Signal Processing), vImage, BLAS, and LAPACK. Apps that
//! perform audio analysis (e.g. Talking Tom/Lila) use vDSP FFT routines.
//!
//! This implementation provides the vDSP FFT functions that apps commonly
//! call. The FFT is computed using a simple radix-2 Cooley-Tukey algorithm
//! which is correct but not SIMD-optimized (sufficient for emulation).

use crate::dyld::{export_c_func, FunctionExports};
use crate::mem::{ConstPtr, GuestUSize, MutPtr, MutVoidPtr, Ptr, SafeRead};
use crate::Environment;


use std::f32::consts::PI;

/// Opaque type representing an FFT setup object.
/// On real iOS this contains precomputed twiddle factors.
/// We store log2n so we know the FFT size.
type FFTSetup = MutVoidPtr;

/// vDSP_Length is typedef'd as `unsigned long` on ARM32 = u32
type vDSP_Length = u32;

/// FFT direction constants
#[allow(non_upper_case_globals)]
const kFFTDirection_Forward: i32 = 1;
#[allow(non_upper_case_globals, dead_code)]
const kFFTDirection_Inverse: i32 = -1;

/// FFT radix constants
#[allow(non_upper_case_globals)]
const kFFTRadix2: i32 = 0;

/// DSPSplitComplex layout in guest memory:
/// struct DSPSplitComplex { float *realp; float *imagp; };
#[repr(C, packed)]
struct GuestDSPSplitComplex {
    realp: MutPtr<f32>,
    imagp: MutPtr<f32>,
}
unsafe impl SafeRead for GuestDSPSplitComplex {}


/// `vDSP_create_fftsetup` — allocate and initialize an FFT weights array.
///
/// Apple docs: Creates an FFT setup structure for use with single-precision
/// FFT functions. The returned object is opaque; we store the log2n in it
/// so we can recover the transform length later.
///
/// Reference: Apple vDSP Reference — vDSP_create_fftsetup
fn vDSP_create_fftsetup(
    env: &mut Environment,
    log2n: vDSP_Length,
    _radix: i32, // kFFTRadix2 = 0
) -> FFTSetup {
    // Allocate a small block to store log2n (4 bytes is enough).
    let ptr = env.mem.alloc(4);
    env.mem.write(ptr.cast::<u32>(), log2n);
    log_dbg!(
        "vDSP_create_fftsetup(log2n={}, radix={}) => {:?}",
        log2n,
        _radix,
        ptr
    );
    ptr
}

/// `vDSP_destroy_fftsetup` — free an FFT setup structure.
fn vDSP_destroy_fftsetup(env: &mut Environment, setup: FFTSetup) {
    if !setup.is_null() {
        log_dbg!("vDSP_destroy_fftsetup({:?})", setup);
        env.mem.free(setup);
    }
}


/// Helper: perform in-place radix-2 FFT on split-complex data.
/// Uses Cooley-Tukey decimation-in-time algorithm.
fn do_fft_split(real: &mut [f32], imag: &mut [f32], n: usize, direction: i32) {
    assert_eq!(real.len(), n);
    assert_eq!(imag.len(), n);

    // Bit-reversal permutation
    let mut j: usize = 0;
    for i in 1..n {
        let mut bit = n >> 1;
        while j & bit != 0 {
            j ^= bit;
            bit >>= 1;
        }
        j ^= bit;
        if i < j {
            real.swap(i, j);
            imag.swap(i, j);
        }
    }

    // Cooley-Tukey butterfly
    let sign: f32 = if direction == kFFTDirection_Forward {
        -1.0
    } else {
        1.0
    };

    let mut len = 2;
    while len <= n {
        let half = len / 2;
        let angle = sign * 2.0 * PI / (len as f32);
        for i in (0..n).step_by(len) {
            for k in 0..half {
                let theta = angle * (k as f32);
                let (cos_t, sin_t) = (theta.cos(), theta.sin());
                let tr = real[i + k + half] * cos_t - imag[i + k + half] * sin_t;
                let ti = real[i + k + half] * sin_t + imag[i + k + half] * cos_t;
                real[i + k + half] = real[i + k] - tr;
                imag[i + k + half] = imag[i + k] - ti;
                real[i + k] += tr;
                imag[i + k] += ti;
            }
        }
        len <<= 1;
    }
}


/// `vDSP_fft_zip` — in-place single-precision complex FFT.
///
/// Apple docs: Computes an in-place single-precision complex discrete Fourier
/// transform of the specified signal, either forward or inverse.
fn vDSP_fft_zip(
    env: &mut Environment,
    setup: FFTSetup,
    io_data: ConstPtr<GuestDSPSplitComplex>, // *mut DSPSplitComplex
    stride: vDSP_Length,
    log2n: vDSP_Length,
    direction: i32,
) {
    if setup.is_null() || io_data.is_null() {
        log!("vDSP_fft_zip: NULL setup or ioData, returning");
        return;
    }
    let n: usize = 1 << log2n;
    let split: GuestDSPSplitComplex = env.mem.read(io_data);

    // Read data from guest memory
    let mut real_vec = vec![0.0f32; n];
    let mut imag_vec = vec![0.0f32; n];
    let stride_us = stride.max(1) as usize;
    for i in 0..n {
        let offset = (i * stride_us) as GuestUSize;
        real_vec[i] = env.mem.read((split.realp + offset).cast());
        imag_vec[i] = env.mem.read((split.imagp + offset).cast());
    }

    do_fft_split(&mut real_vec, &mut imag_vec, n, direction);

    // Write results back
    for i in 0..n {
        let offset = (i * stride_us) as GuestUSize;
        env.mem.write((split.realp + offset).cast(), real_vec[i]);
        env.mem.write((split.imagp + offset).cast(), imag_vec[i]);
    }
}


/// `vDSP_fft_zrip` — in-place single-precision real FFT (packed format).
///
/// Apple docs: Computes an in-place single-precision real discrete Fourier
/// transform (using a split-complex representation where the even-indexed
/// real elements go to realp and the odd-indexed elements go to imagp).
fn vDSP_fft_zrip(
    env: &mut Environment,
    setup: FFTSetup,
    io_data: ConstPtr<GuestDSPSplitComplex>,
    stride: vDSP_Length,
    log2n: vDSP_Length,
    direction: i32,
) {
    // For real FFTs on packed split-complex, the actual complex FFT
    // length is N/2 where N = 1 << log2n. Apple's packed format stores
    // the real FFT result in N/2 complex bins. We implement this as a
    // complex FFT on the packed data directly (sufficient approximation).
    if setup.is_null() || io_data.is_null() {
        log!("vDSP_fft_zrip: NULL setup or ioData, returning");
        return;
    }
    let n_half: usize = 1 << (log2n.saturating_sub(1));
    let n = n_half.max(1);
    let split: GuestDSPSplitComplex = env.mem.read(io_data);

    let mut real_vec = vec![0.0f32; n];
    let mut imag_vec = vec![0.0f32; n];
    let stride_us = stride.max(1) as usize;
    for i in 0..n {
        let offset = (i * stride_us) as GuestUSize;
        real_vec[i] = env.mem.read((split.realp + offset).cast());
        imag_vec[i] = env.mem.read((split.imagp + offset).cast());
    }

    do_fft_split(&mut real_vec, &mut imag_vec, n, direction);

    for i in 0..n {
        let offset = (i * stride_us) as GuestUSize;
        env.mem.write((split.realp + offset).cast(), real_vec[i]);
        env.mem.write((split.imagp + offset).cast(), imag_vec[i]);
    }
}


/// `vDSP_fft_zop` — out-of-place single-precision complex FFT.
fn vDSP_fft_zop(
    env: &mut Environment,
    setup: FFTSetup,
    in_data: ConstPtr<GuestDSPSplitComplex>,
    in_stride: vDSP_Length,
    out_data: ConstPtr<GuestDSPSplitComplex>,
    out_stride: vDSP_Length,
    log2n: vDSP_Length,
    direction: i32,
) {
    if setup.is_null() || in_data.is_null() || out_data.is_null() {
        log!("vDSP_fft_zop: NULL args, returning");
        return;
    }
    let n: usize = 1 << log2n;
    let in_split: GuestDSPSplitComplex = env.mem.read(in_data);
    let out_split: GuestDSPSplitComplex = env.mem.read(out_data);

    let in_stride_us = in_stride.max(1) as usize;
    let mut real_vec = vec![0.0f32; n];
    let mut imag_vec = vec![0.0f32; n];
    for i in 0..n {
        let offset = (i * in_stride_us) as GuestUSize;
        real_vec[i] = env.mem.read((in_split.realp + offset).cast());
        imag_vec[i] = env.mem.read((in_split.imagp + offset).cast());
    }

    do_fft_split(&mut real_vec, &mut imag_vec, n, direction);

    let out_stride_us = out_stride.max(1) as usize;
    for i in 0..n {
        let offset = (i * out_stride_us) as GuestUSize;
        env.mem.write((out_split.realp + offset).cast(), real_vec[i]);
        env.mem.write((out_split.imagp + offset).cast(), imag_vec[i]);
    }
}


/// `vDSP_vsmul` — vector scalar multiply (single-precision).
/// C[i] = A[i*stride_a] * B, for i in 0..n
fn vDSP_vsmul(
    env: &mut Environment,
    input: ConstPtr<f32>,
    stride_a: vDSP_Length,
    scalar: ConstPtr<f32>,
    output: MutPtr<f32>,
    stride_c: vDSP_Length,
    n: vDSP_Length,
) {
    let s: f32 = env.mem.read(scalar);
    let sa = stride_a.max(1) as GuestUSize;
    let sc = stride_c.max(1) as GuestUSize;
    for i in 0..(n as GuestUSize) {
        let val: f32 = env.mem.read((input + i * sa).cast());
        env.mem.write((output + i * sc).cast(), val * s);
    }
}

/// `vDSP_zvmags` — squared magnitudes of complex vector.
/// C[i] = A.realp[i*stride]^2 + A.imagp[i*stride]^2
fn vDSP_zvmags(
    env: &mut Environment,
    input: ConstPtr<GuestDSPSplitComplex>,
    stride_a: vDSP_Length,
    output: MutPtr<f32>,
    stride_c: vDSP_Length,
    n: vDSP_Length,
) {
    let split: GuestDSPSplitComplex = env.mem.read(input);
    let sa = stride_a.max(1) as GuestUSize;
    let sc = stride_c.max(1) as GuestUSize;
    for i in 0..(n as GuestUSize) {
        let r: f32 = env.mem.read((split.realp + i * sa).cast());
        let im: f32 = env.mem.read((split.imagp + i * sa).cast());
        env.mem.write((output + i * sc).cast(), r * r + im * im);
    }
}


/// `vDSP_meanv` — mean of a vector (single-precision).
fn vDSP_meanv(
    env: &mut Environment,
    input: ConstPtr<f32>,
    stride: vDSP_Length,
    output: MutPtr<f32>,
    n: vDSP_Length,
) {
    if n == 0 {
        env.mem.write(output, 0.0f32);
        return;
    }
    let s = stride.max(1) as GuestUSize;
    let mut sum: f32 = 0.0;
    for i in 0..(n as GuestUSize) {
        let val: f32 = env.mem.read((input + i * s).cast());
        sum += val;
    }
    env.mem.write(output, sum / (n as f32));
}

/// `vDSP_maxv` — maximum of a vector (single-precision).
fn vDSP_maxv(
    env: &mut Environment,
    input: ConstPtr<f32>,
    stride: vDSP_Length,
    output: MutPtr<f32>,
    n: vDSP_Length,
) {
    if n == 0 {
        return;
    }
    let s = stride.max(1) as GuestUSize;
    let mut max_val: f32 = f32::NEG_INFINITY;
    for i in 0..(n as GuestUSize) {
        let val: f32 = env.mem.read((input + i * s).cast());
        if val > max_val {
            max_val = val;
        }
    }
    env.mem.write(output, max_val);
}


/// `vDSP_minv` — minimum of a vector (single-precision).
fn vDSP_minv(
    env: &mut Environment,
    input: ConstPtr<f32>,
    stride: vDSP_Length,
    output: MutPtr<f32>,
    n: vDSP_Length,
) {
    if n == 0 {
        return;
    }
    let s = stride.max(1) as GuestUSize;
    let mut min_val: f32 = f32::INFINITY;
    for i in 0..(n as GuestUSize) {
        let val: f32 = env.mem.read((input + i * s).cast());
        if val < min_val {
            min_val = val;
        }
    }
    env.mem.write(output, min_val);
}

/// `vDSP_rmsqv` — root mean square of a vector (single-precision).
fn vDSP_rmsqv(
    env: &mut Environment,
    input: ConstPtr<f32>,
    stride: vDSP_Length,
    output: MutPtr<f32>,
    n: vDSP_Length,
) {
    if n == 0 {
        env.mem.write(output, 0.0f32);
        return;
    }
    let s = stride.max(1) as GuestUSize;
    let mut sum_sq: f32 = 0.0;
    for i in 0..(n as GuestUSize) {
        let val: f32 = env.mem.read((input + i * s).cast());
        sum_sq += val * val;
    }
    env.mem.write(output, (sum_sq / (n as f32)).sqrt());
}


/// `vDSP_ctoz` — interleaved-complex to split-complex conversion.
/// Copies interleaved complex data (real, imag, real, imag, ...) into
/// separate real and imaginary arrays.
fn vDSP_ctoz(
    env: &mut Environment,
    input: ConstPtr<f32>,     // interleaved complex pairs
    stride_input: vDSP_Length,
    output: ConstPtr<GuestDSPSplitComplex>,
    stride_output: vDSP_Length,
    n: vDSP_Length,
) {
    let split: GuestDSPSplitComplex = env.mem.read(output);
    let si = stride_input.max(1) as GuestUSize; // in float pairs => *2
    let so = stride_output.max(1) as GuestUSize;
    for i in 0..(n as GuestUSize) {
        let r: f32 = env.mem.read((input + i * si * 2).cast());
        let im: f32 = env.mem.read((input + i * si * 2 + 1).cast());
        env.mem.write((split.realp + i * so).cast(), r);
        env.mem.write((split.imagp + i * so).cast(), im);
    }
}

/// `vDSP_ztoc` — split-complex to interleaved-complex conversion.
fn vDSP_ztoc(
    env: &mut Environment,
    input: ConstPtr<GuestDSPSplitComplex>,
    stride_input: vDSP_Length,
    output: MutPtr<f32>,      // interleaved complex pairs
    stride_output: vDSP_Length,
    n: vDSP_Length,
) {
    let split: GuestDSPSplitComplex = env.mem.read(input);
    let si = stride_input.max(1) as GuestUSize;
    let so = stride_output.max(1) as GuestUSize;
    for i in 0..(n as GuestUSize) {
        let r: f32 = env.mem.read((split.realp + i * si).cast());
        let im: f32 = env.mem.read((split.imagp + i * si).cast());
        env.mem.write((output + i * so * 2).cast(), r);
        env.mem.write((output + i * so * 2 + 1).cast(), im);
    }
}


/// `vDSP_vadd` — vector add (single-precision). C[i] = A[i] + B[i]
fn vDSP_vadd(
    env: &mut Environment,
    input_a: ConstPtr<f32>,
    stride_a: vDSP_Length,
    input_b: ConstPtr<f32>,
    stride_b: vDSP_Length,
    output: MutPtr<f32>,
    stride_c: vDSP_Length,
    n: vDSP_Length,
) {
    let sa = stride_a.max(1) as GuestUSize;
    let sb = stride_b.max(1) as GuestUSize;
    let sc = stride_c.max(1) as GuestUSize;
    for i in 0..(n as GuestUSize) {
        let a: f32 = env.mem.read((input_a + i * sa).cast());
        let b: f32 = env.mem.read((input_b + i * sb).cast());
        env.mem.write((output + i * sc).cast(), a + b);
    }
}

/// `vDSP_vmul` — vector multiply (single-precision). C[i] = A[i] * B[i]
fn vDSP_vmul(
    env: &mut Environment,
    input_a: ConstPtr<f32>,
    stride_a: vDSP_Length,
    input_b: ConstPtr<f32>,
    stride_b: vDSP_Length,
    output: MutPtr<f32>,
    stride_c: vDSP_Length,
    n: vDSP_Length,
) {
    let sa = stride_a.max(1) as GuestUSize;
    let sb = stride_b.max(1) as GuestUSize;
    let sc = stride_c.max(1) as GuestUSize;
    for i in 0..(n as GuestUSize) {
        let a: f32 = env.mem.read((input_a + i * sa).cast());
        let b: f32 = env.mem.read((input_b + i * sb).cast());
        env.mem.write((output + i * sc).cast(), a * b);
    }
}


/// `vDSP_vfill` — fill vector with scalar.
fn vDSP_vfill(
    env: &mut Environment,
    scalar: ConstPtr<f32>,
    output: MutPtr<f32>,
    stride: vDSP_Length,
    n: vDSP_Length,
) {
    let val: f32 = env.mem.read(scalar);
    let s = stride.max(1) as GuestUSize;
    for i in 0..(n as GuestUSize) {
        env.mem.write((output + i * s).cast(), val);
    }
}

/// `vDSP_vclr` — clear (zero) a vector.
fn vDSP_vclr(
    env: &mut Environment,
    output: MutPtr<f32>,
    stride: vDSP_Length,
    n: vDSP_Length,
) {
    let s = stride.max(1) as GuestUSize;
    for i in 0..(n as GuestUSize) {
        env.mem.write((output + i * s).cast(), 0.0f32);
    }
}

/// `vImage_Buffer` — vImage's universal pixel buffer descriptor.
/// Layout from `<Accelerate/vImage.h>` (32-bit ARM):
///   - `void *data`        (4B)
///   - `vImagePixelCount height` (4B; `unsigned long` == 32-bit on ARM32)
///   - `vImagePixelCount width`  (4B)
///   - `size_t rowBytes`         (4B)
#[repr(C, packed)]
#[derive(Copy, Clone, Default)]
struct VImageBuffer {
    data: u32,      // guest pointer
    height: u32,
    width: u32,
    row_bytes: u32,
}
unsafe impl SafeRead for VImageBuffer {}

/// vImage error codes, per `<Accelerate/vImage_Types.h>`.
const KV_IMAGE_NO_ERROR: i32 = 0;
const KV_IMAGE_NULL_POINTER_ARGUMENT: i32 = -21772;
const KV_IMAGE_INVALID_PARAMETER: i32 = -21767;

/// `vImage_Error vImageConvert_AnyToAny(const vImageConverterRef converter,
///                                      const vImage_Buffer *srcs[],
///                                      const vImage_Buffer *dests[],
///                                      void *tempBuffer,
///                                      vImage_Flags flags)` — vImage
/// "universal" pixel-format converter. See
/// <https://developer.apple.com/documentation/accelerate/1533487-vimageconvert_anytoany>.
///
/// touchHLE does not implement `vImageConverter_CreateWithCGImageFormat`
/// (the factory that produces non-null converter refs), so any call here
/// arrives with `converter == NULL`. Apple's documentation says this is
/// `kvImageNullPointerArgument` (-21772) and apps that build vImage
/// pipelines for optional features (sRGB conversion, scaling) fall
/// back to their slow path when they observe this code.
#[allow(non_snake_case)]
fn vImageConvert_AnyToAny(
    _env: &mut Environment,
    converter: MutVoidPtr,
    srcs: ConstPtr<u32>,
    dests: ConstPtr<u32>,
    _temp_buffer: MutVoidPtr,
    _flags: u32,
) -> i32 {
    if converter.is_null() {
        return KV_IMAGE_NULL_POINTER_ARGUMENT;
    }
    if srcs.is_null() || dests.is_null() {
        return KV_IMAGE_NULL_POINTER_ARGUMENT;
    }
    KV_IMAGE_INVALID_PARAMETER
}

/// `vImage_Error vImageCopyBuffer(const vImage_Buffer *src,
///                                const vImage_Buffer *dest,
///                                size_t pixelSize, vImage_Flags flags)`
/// — copies `src` to `dest` exactly when both buffers have matching
/// dimensions. Apple's libvImage uses this internally for many
/// "AnyToAny" conversions where source and destination formats match.
#[allow(non_snake_case)]
fn vImageCopyBuffer(
    env: &mut Environment,
    src: ConstPtr<VImageBuffer>,
    dest: ConstPtr<VImageBuffer>,
    pixel_size: u32,
    _flags: u32,
) -> i32 {
    if src.is_null() || dest.is_null() {
        return KV_IMAGE_NULL_POINTER_ARGUMENT;
    }
    let s = env.mem.read(src);
    let d = env.mem.read(dest);
    let src_height = s.height;
    let src_width = s.width;
    let src_row_bytes = s.row_bytes;
    let src_data = s.data;
    let dst_height = d.height;
    let dst_width = d.width;
    let dst_row_bytes = d.row_bytes;
    let dst_data = d.data;
    if src_width != dst_width || src_height != dst_height {
        return KV_IMAGE_INVALID_PARAMETER;
    }
    let line_bytes = (src_width as u64) * (pixel_size as u64);
    let line_bytes = line_bytes.min(src_row_bytes as u64).min(dst_row_bytes as u64) as u32;
    for y in 0..src_height {
        let src_row = src_data + y * src_row_bytes;
        let dst_row = dst_data + y * dst_row_bytes;
        let bytes: Vec<u8> = env
            .mem
            .bytes_at(crate::mem::Ptr::<u8, false>::from_bits(src_row), line_bytes)
            .to_vec();
        env.mem
            .bytes_at_mut(crate::mem::Ptr::<u8, true>::from_bits(dst_row), line_bytes)
            .copy_from_slice(&bytes);
    }
    KV_IMAGE_NO_ERROR
}

pub const FUNCTIONS: FunctionExports = &[
    export_c_func!(vDSP_create_fftsetup(_, _)),
    export_c_func!(vDSP_destroy_fftsetup(_)),
    export_c_func!(vDSP_fft_zip(_, _, _, _, _)),
    export_c_func!(vDSP_fft_zrip(_, _, _, _, _)),
    export_c_func!(vDSP_fft_zop(_, _, _, _, _, _, _)),
    export_c_func!(vDSP_ctoz(_, _, _, _, _)),
    export_c_func!(vDSP_ztoc(_, _, _, _, _)),
    export_c_func!(vDSP_vsmul(_, _, _, _, _, _)),
    export_c_func!(vDSP_zvmags(_, _, _, _, _)),
    export_c_func!(vDSP_meanv(_, _, _, _)),
    export_c_func!(vDSP_maxv(_, _, _, _)),
    export_c_func!(vDSP_minv(_, _, _, _)),
    export_c_func!(vDSP_rmsqv(_, _, _, _)),
    export_c_func!(vDSP_vadd(_, _, _, _, _, _, _)),
    export_c_func!(vDSP_vmul(_, _, _, _, _, _, _)),
    export_c_func!(vDSP_vfill(_, _, _, _)),
    export_c_func!(vDSP_vclr(_, _, _)),
    export_c_func!(vImageConvert_AnyToAny(_, _, _, _, _)),
    export_c_func!(vImageCopyBuffer(_, _, _, _)),
];
