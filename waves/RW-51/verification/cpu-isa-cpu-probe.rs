// Diagnostic only. Uses standard OS-aware Rust feature detection; no handwritten SIMD.
fn main() {
    println!("architecture={}", std::env::consts::ARCH);
    println!("mmx={}", std::arch::is_x86_feature_detected!("mmx"));
    println!("sse={}", std::arch::is_x86_feature_detected!("sse"));
    println!("sse2={}", std::arch::is_x86_feature_detected!("sse2"));
    println!("sse3={}", std::arch::is_x86_feature_detected!("sse3"));
    println!("ssse3={}", std::arch::is_x86_feature_detected!("ssse3"));
    println!("sse4.1={}", std::arch::is_x86_feature_detected!("sse4.1"));
    println!("sse4.2={}", std::arch::is_x86_feature_detected!("sse4.2"));
    println!("sse4a={}", std::arch::is_x86_feature_detected!("sse4a"));
    println!("aes={}", std::arch::is_x86_feature_detected!("aes"));
    println!("avx={}", std::arch::is_x86_feature_detected!("avx"));
    println!("avx2={}", std::arch::is_x86_feature_detected!("avx2"));
    println!("avxvnni={}", std::arch::is_x86_feature_detected!("avxvnni"));
    println!("fma={}", std::arch::is_x86_feature_detected!("fma"));
    println!("sha={}", std::arch::is_x86_feature_detected!("sha"));
    println!("avx512f={}", std::arch::is_x86_feature_detected!("avx512f"));
    println!("avx512cd={}", std::arch::is_x86_feature_detected!("avx512cd"));
    println!("avx512er={}", std::arch::is_x86_feature_detected!("avx512er"));
    println!("avx512pf={}", std::arch::is_x86_feature_detected!("avx512pf"));
    println!("avx512bw={}", std::arch::is_x86_feature_detected!("avx512bw"));
    println!("avx512dq={}", std::arch::is_x86_feature_detected!("avx512dq"));
    println!("avx512vl={}", std::arch::is_x86_feature_detected!("avx512vl"));
    println!("avx512ifma={}", std::arch::is_x86_feature_detected!("avx512ifma"));
    println!("avx512vbmi={}", std::arch::is_x86_feature_detected!("avx512vbmi"));
    println!("avx512vbmi2={}", std::arch::is_x86_feature_detected!("avx512vbmi2"));
    println!("avx512vpopcntdq={}", std::arch::is_x86_feature_detected!("avx512vpopcntdq"));
    println!("avx512vnni={}", std::arch::is_x86_feature_detected!("avx512vnni"));
    println!("avx512bitalg={}", std::arch::is_x86_feature_detected!("avx512bitalg"));
    println!("avx512bf16={}", std::arch::is_x86_feature_detected!("avx512bf16"));
    println!("avx512vp2intersect={}", std::arch::is_x86_feature_detected!("avx512vp2intersect"));
    println!("avx512fp16={}", std::arch::is_x86_feature_detected!("avx512fp16"));
}
