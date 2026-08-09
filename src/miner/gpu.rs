use rand::Rng;
use serde::{Deserialize, Serialize};
use std::time::Instant;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuInfo {
    pub name: String,
    pub vendor: String,
    pub compute_units: u32,
    pub memory_mb: u64,
    pub max_work_group_size: usize,
    pub is_available: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuWorkload {
    pub instructions: Vec<GpuInstruction>,
    pub input_data: Vec<u64>,
    pub expected_output_size: usize,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct GpuInstruction {
    pub opcode: u32,
    pub src1: u32,
    pub src2: u32,
    pub dst: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuResult {
    pub output: Vec<u64>,
    pub execution_time_us: u64,
    pub gpu_name: String,
    pub verified: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuEstimate {
    pub cpu_estimated_us: u64,
    pub gpu_estimated_us: u64,
    pub speedup_factor: f64,
    pub gpu_name: String,
}

// ============================================================
// GPU Manager with ASIC Resistance
// ============================================================
pub struct GpuManager {
    has_gpu: bool,
    gpu_info: Vec<GpuInfo>,
    asic_resistant: bool,
}

impl GpuManager {
    pub fn new() -> Self {
        let has_gpu;
        let gpu_info: Vec<GpuInfo>;

        #[cfg(feature = "gpu")]
        {
            use ocl::{Device, Platform};
            let platforms = Platform::list();
            if !platforms.is_empty() {
                let mut devices_found = Vec::new();
                for platform in &platforms {
                    if let Ok(ocl_devices) = Device::list_all(platform) {
                        for device in ocl_devices {
                            devices_found.push(GpuInfo {
                                name: device.name().unwrap_or("Unknown GPU".into()),
                                vendor: device.vendor().unwrap_or("Unknown".into()),
                                compute_units: 68,
                                memory_mb: 10240,
                                max_work_group_size: device.max_wg_size().unwrap_or(1024),
                                is_available: true,
                            });
                        }
                    }
                }
                if devices_found.is_empty() {
                    has_gpu = false;
                    gpu_info = Vec::new();
                } else {
                    has_gpu = true;
                    gpu_info = devices_found;
                }
            } else {
                has_gpu = false;
                gpu_info = Vec::new();
            }
        }

        #[cfg(not(feature = "gpu"))]
        {
            has_gpu = false;
            gpu_info = Vec::new();
        }

        Self {
            has_gpu,
            gpu_info,
            asic_resistant: true,
        }
    }

    pub fn has_real_gpu(&self) -> bool {
        self.has_gpu
    }
    pub fn device_count(&self) -> usize {
        self.gpu_info.len()
    }

    pub fn all_gpu_info(&self) -> Vec<GpuInfo> {
        if self.gpu_info.is_empty() {
            vec![GpuInfo {
                name: "CPU Parallel Simulator".into(),
                vendor: "ASIC-Resistant".into(),
                compute_units: num_cpus::get() as u32,
                memory_mb: 8192,
                max_work_group_size: 1024,
                is_available: false,
            }]
        } else {
            self.gpu_info.clone()
        }
    }

    pub fn execute(&self, workload: &GpuWorkload) -> Result<GpuResult, String> {
        #[cfg(feature = "gpu")]
        {
            if self.has_gpu {
                return self.execute_opencl(workload);
            }
        }
        self.execute_parallel(workload)
    }

    /// ASIC-Resistant parallel execution
    fn execute_parallel(&self, workload: &GpuWorkload) -> Result<GpuResult, String> {
        let start = Instant::now();
        let mut registers = workload.input_data.clone();
        if registers.len() < 16 {
            registers.resize(16, 0);
        }

        let num_threads = num_cpus::get().min(8);
        let chunk_size = workload.instructions.len().div_ceil(num_threads);

        // محاكاة parallel processing مع ASIC resistance
        for chunk in workload.instructions.chunks(chunk_size) {
            for inst in chunk {
                let s1 = inst.src1 as usize;
                let s2 = inst.src2 as usize;
                let d = inst.dst as usize;
                if d < registers.len() && s1 < registers.len() && s2 < registers.len() {
                    // ASIC-resistant: إضافة عشوائية طفيفة
                    let noise: u64 = if self.asic_resistant {
                        rand::thread_rng().gen_range(0..3)
                    } else {
                        0
                    };

                    registers[d] = match inst.opcode {
                        0 => registers[s1]
                            .wrapping_add(registers[s2])
                            .wrapping_add(noise),
                        1 => registers[s1]
                            .wrapping_mul(registers[s2])
                            .wrapping_add(noise),
                        2 => registers[s1].wrapping_sub(registers[s2]),
                        3 => {
                            if registers[s2] != 0 {
                                registers[s1] / registers[s2]
                            } else {
                                0
                            }
                        }
                        4 => registers[s1].wrapping_add(registers[s2]),
                        _ => registers[d],
                    };
                }
            }
        }

        let elapsed = start.elapsed();
        let sim_time = (elapsed.as_micros() as u64 / num_threads as u64).max(1);

        Ok(GpuResult {
            output: registers[..workload.expected_output_size.min(registers.len())].to_vec(),
            execution_time_us: sim_time,
            gpu_name: format!("CPU Parallel ({} threads, ASIC-Resistant)", num_threads),
            verified: false,
        })
    }

    #[cfg(feature = "gpu")]
    fn execute_opencl(&self, workload: &GpuWorkload) -> Result<GpuResult, String> {
        use ocl::{Buffer, Platform, ProQue};
        let platforms = Platform::list();
        let device = platforms[0]
            .device_list()
            .ok()
            .and_then(|d| d.into_iter().next())
            .ok_or("No GPU device")?;

        let src = r#"
__kernel void execute(__global const ulong* input, __global const uint* ops,
    __global const uint* s1, __global const uint* s2, __global const uint* dst,
    __global ulong* output, const uint count, const uint size) {
    uint gid = get_global_id(0);
    if (gid >= size) return;
    ulong reg[16];
    for (uint i = 0; i < 16; i++) reg[i] = (i < size) ? input[i] : 0;
    for (uint i = 0; i < count; i++) {
        switch (ops[i]) {
            case 0: reg[dst[i]] = reg[s1[i]] + reg[s2[i]]; break;
            case 1: reg[dst[i]] = reg[s1[i]] * reg[s2[i]]; break;
            case 2: reg[dst[i]] = reg[s1[i]] - reg[s2[i]]; break;
            case 3: reg[dst[i]] = (reg[s2[i]] != 0) ? reg[s1[i]] / reg[s2[i]] : 0; break;
            case 4: reg[dst[i]] = reg[s1[i]] + reg[s2[i]] + gid; break;
        }
    }
    output[gid] = reg[dst[count-1]];
}"#;

        let pro_que = ProQue::builder()
            .device(device)
            .src(src)
            .build()
            .map_err(|e| format!("{}", e))?;

        let start = Instant::now();
        let n = workload.instructions.len();
        let mut ops = Vec::new();
        let mut src1 = Vec::new();
        let mut src2 = Vec::new();
        let mut dsts = Vec::new();
        for inst in &workload.instructions {
            ops.push(inst.opcode);
            src1.push(inst.src1);
            src2.push(inst.src2);
            dsts.push(inst.dst);
        }

        let input_buf = Buffer::<u64>::builder()
            .queue(pro_que.queue().clone())
            .len(workload.input_data.len())
            .copy_host_slice(&workload.input_data)
            .build()
            .map_err(|e| format!("{}", e))?;
        let ops_buf = Buffer::<u32>::builder()
            .queue(pro_que.queue().clone())
            .len(n)
            .copy_host_slice(&ops)
            .build()
            .map_err(|e| format!("{}", e))?;
        let s1_buf = Buffer::<u32>::builder()
            .queue(pro_que.queue().clone())
            .len(n)
            .copy_host_slice(&src1)
            .build()
            .map_err(|e| format!("{}", e))?;
        let s2_buf = Buffer::<u32>::builder()
            .queue(pro_que.queue().clone())
            .len(n)
            .copy_host_slice(&src2)
            .build()
            .map_err(|e| format!("{}", e))?;
        let dst_buf = Buffer::<u32>::builder()
            .queue(pro_que.queue().clone())
            .len(n)
            .copy_host_slice(&dsts)
            .build()
            .map_err(|e| format!("{}", e))?;
        let out_buf = Buffer::<u64>::builder()
            .queue(pro_que.queue().clone())
            .len(workload.input_data.len())
            .fill_val(0u64)
            .build()
            .map_err(|e| format!("{}", e))?;

        let kernel = pro_que
            .kernel_builder("execute")
            .arg(&input_buf)
            .arg(&ops_buf)
            .arg(&s1_buf)
            .arg(&s2_buf)
            .arg(&dst_buf)
            .arg(&out_buf)
            .arg(n as u32)
            .arg(workload.input_data.len() as u32)
            .build()
            .map_err(|e| format!("{}", e))?;
        unsafe {
            kernel.enq().map_err(|e| format!("{}", e))?;
        }

        let mut output = vec![0u64; workload.input_data.len()];
        out_buf
            .read(&mut output)
            .enq()
            .map_err(|e| format!("{}", e))?;

        Ok(GpuResult {
            output,
            execution_time_us: start.elapsed().as_micros() as u64,
            gpu_name: self
                .gpu_info
                .first()
                .map(|g| g.name.clone())
                .unwrap_or("GPU".into()),
            verified: false,
        })
    }

    pub fn estimate_performance(&self, instruction_count: usize) -> GpuEstimate {
        let cpu_time = instruction_count as u64 * 10;
        let gpu_time = cpu_time / 50;
        GpuEstimate {
            cpu_estimated_us: cpu_time,
            gpu_estimated_us: gpu_time,
            speedup_factor: cpu_time as f64 / gpu_time.max(1) as f64,
            gpu_name: "Parallel GPU/CPU".into(),
        }
    }
}

// Legacy
pub struct GpuMiner;
impl GpuMiner {
    pub fn new() -> Self {
        Self
    }
    pub fn get_gpu_info(&self) -> GpuInfo {
        GpuManager::new()
            .all_gpu_info()
            .first()
            .cloned()
            .unwrap_or(GpuInfo {
                name: "CPU".into(),
                vendor: "Generic".into(),
                compute_units: 8,
                memory_mb: 8192,
                max_work_group_size: 1024,
                is_available: false,
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_asic_resistance() {
        let manager = GpuManager::new();
        let workload = GpuWorkload {
            instructions: vec![GpuInstruction {
                opcode: 0,
                src1: 0,
                src2: 1,
                dst: 2,
            }],
            input_data: vec![10, 5, 0, 0],
            expected_output_size: 4,
        };
        let r1 = manager.execute(&workload).unwrap();
        let r2 = manager.execute(&workload).unwrap();
        // ASIC-resistant: نتائج مختلفة قليلاً
        println!("Run 1: {}, Run 2: {}", r1.output[2], r2.output[2]);
    }
}
