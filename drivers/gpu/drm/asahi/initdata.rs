// SPDX-License-Identifier: GPL-2.0-only OR MIT
#![allow(missing_docs)]
#![allow(dead_code)]
#![allow(clippy::unusual_byte_groupings)]

//! GPU initialization data builder

use crate::debug::*;
use crate::fw::initdata::*;
use crate::fw::types::*;
use crate::{box_in_place, f32, place};
use crate::{gpu, hw, mmu};
use kernel::error::Result;
use kernel::macros::versions;

const DEBUG_CLASS: DebugFlags = DebugFlags::Init;

#[versions(AGX)]
pub(crate) struct InitDataBuilder<'a> {
    alloc: &'a mut gpu::KernelAllocators,
    cfg: &'a hw::HwConfig,
    dyncfg: &'a hw::DynConfig,
}

#[versions(AGX)]
impl<'a> InitDataBuilder::ver<'a> {
    pub(crate) fn new(
        alloc: &'a mut gpu::KernelAllocators,
        cfg: &'a hw::HwConfig,
        dyncfg: &'a hw::DynConfig,
    ) -> InitDataBuilder::ver<'a> {
        InitDataBuilder::ver { alloc, cfg, dyncfg }
    }

    #[inline(never)]
    fn hw_shared1(cfg: &hw::HwConfig) -> raw::HwDataShared1 {
        let mut ret = raw::HwDataShared1 {
            unk_a4: cfg.shared1_a4,
            ..Default::default()
        };
        for (i, val) in cfg.shared1_tab.iter().enumerate() {
            ret.table[i] = *val;
        }
        ret
    }

    fn init_curve(
        curve: &mut raw::HwDataShared2Curve,
        unk_0: u32,
        unk_4: u32,
        t1: &[i16],
        t2: &[i16],
        t3: &[&[i32]],
    ) {
        curve.unk_0 = unk_0;
        curve.unk_4 = unk_4;
        (*curve.t1)[..t1.len()].copy_from_slice(t1);
        (*curve.t1)[t1.len()..].fill(t1[0]);
        (*curve.t2)[..t2.len()].copy_from_slice(t2);
        (*curve.t2)[t2.len()..].fill(t2[0]);
        for (i, a) in curve.t3.iter_mut().enumerate() {
            a.fill(0x3ffffff);
            if i < t3.len() {
                let b = t3[i];
                (**a)[..b.len()].copy_from_slice(b);
            }
        }
    }

    #[inline(never)]
    fn hw_shared2(cfg: &hw::HwConfig) -> Result<Box<raw::HwDataShared2>> {
        let mut ret = box_in_place!(raw::HwDataShared2 {
            unk_28: Array::new([0xff; 16]),
            t8112: Default::default(),
            unk_508: cfg.shared2_unk_508,
            ..Default::default()
        })?;

        for (i, val) in cfg.shared2_tab.iter().enumerate() {
            ret.table[i] = *val;
        }

        if cfg.chip_id == 0x8112 {
            ret.t8112.unk_14 = 0x6000000;
            Self::init_curve(&mut ret.t8112.curve1, 0, 0x20000000, &[-1], &[0x0f07], &[]);
            Self::init_curve(
                &mut ret.t8112.curve2,
                7,
                0x80000000,
                &[-1, 25740, 17429, 12550, 9597, 7910, 6657, 5881, 5421],
                &[
                    0x0f07, 0x04c0, 0x06c0, 0x08c0, 0x0ac0, 0x0c40, 0x0dc0, 0x0ec0, 0x0f80,
                ],
                &[
                    &[0x3ffffff, 107, 101, 94, 87, 82, 77, 73, 71],
                    &[
                        0x3ffffff, 38240, 36251, 33562, 31368, 29379, 27693, 26211, 25370,
                    ],
                    &[
                        0x3ffffff, 123933, 117485, 108771, 101661, 95217, 89751, 84948, 82222,
                    ],
                ],
            );
        }

        Ok(ret)
    }

    #[inline(never)]
    fn hw_shared3(cfg: &hw::HwConfig) -> Result<Box<raw::HwDataShared3>> {
        let mut ret = box_in_place!(raw::HwDataShared3 {
            ..Default::default()
        })?;

        if cfg.chip_id == 0x8112 {
            ret.unk_0 = 1;
            ret.unk_4 = 500;
            ret.unk_8 = 5;
            ret.table.copy_from_slice(&[
                10700, 10700, 10700, 10700, 10700, 6000, 1000, 1000, 1000, 10700, 10700, 10700,
                10700, 10700, 10700, 10700,
            ]);
            ret.unk_4c = 1;
        }

        Ok(ret)
    }

    fn t81xx_data(dyncfg: &'a hw::DynConfig) -> raw::T81xxData {
        raw::T81xxData {
            unk_d8c: 0x80000000,
            unk_d90: 4,
            unk_d9c: f32!(0.6),
            unk_da4: f32!(0.4),
            unk_dac: f32!(0.38552),
            unk_db8: f32!(65536.0),
            unk_dbc: f32!(13.56),
            max_pstate_scaled: 100 * dyncfg.pwr.perf_max_pstate,
            ..Default::default()
        }
    }

    #[inline(never)]
    fn hwdata_a(&mut self) -> Result<GpuObject<HwDataA::ver>> {
        self.alloc
            .private
            .new_inplace(Default::default(), |_inner, ptr| {
                let pwr = &self.dyncfg.pwr;
                let period_ms = pwr.power_sample_period;
                let period_s = F32::from(period_ms) / f32!(1000.0);
                let ppm_filter_tc_periods = pwr.ppm_filter_time_constant_ms / period_ms;
                let ppm_filter_a = f32!(1.0) / ppm_filter_tc_periods.into();
                let perf_filter_a = f32!(1.0) / pwr.perf_filter_time_constant.into();
                let perf_filter_a2 = f32!(1.0) / pwr.perf_filter_time_constant2.into();
                let avg_power_target_filter_a = f32!(1.0) / pwr.avg_power_target_filter_tc.into();
                let avg_power_filter_tc_periods = pwr.avg_power_filter_tc_ms / period_ms;
                let avg_power_filter_a = f32!(1.0) / avg_power_filter_tc_periods.into();
                let pwr_filter_a = f32!(1.0) / pwr.pwr_filter_time_constant.into();

                let base_ps = pwr.perf_base_pstate;
                let base_ps_scaled = 100 * base_ps;
                let max_ps = pwr.perf_max_pstate;
                let max_ps_scaled = 100 * max_ps;
                let boost_ps_count = max_ps - base_ps;

                let base_clock_khz = self.cfg.base_clock_hz / 1000;
                #[ver(V >= V13_0B4)]
                let base_clock_mhz = self.cfg.base_clock_hz / 1_000_000;
                let clocks_per_period = base_clock_khz * period_ms;

                let raw = place!(
                    ptr,
                    raw::HwDataA::ver {
                        clocks_per_period: clocks_per_period,
                        #[ver(V >= V13_0B4)]
                        clocks_per_period_2: clocks_per_period,
                        pwr_status: AtomicU32::new(4),
                        unk_10: f32!(1.0),
                        actual_pstate: 1,
                        tgt_pstate: 1,
                        base_pstate_scaled: base_ps_scaled,
                        unk_40: 1,
                        max_pstate_scaled: max_ps_scaled,
                        min_pstate_scaled: 100,
                        unk_64c: 625,
                        pwr_filter_a_neg: f32!(1.0) - pwr_filter_a,
                        pwr_filter_a: pwr_filter_a,
                        pwr_integral_gain: pwr.pwr_integral_gain,
                        pwr_integral_min_clamp: pwr.pwr_integral_min_clamp.into(),
                        max_power_1: pwr.max_power_mw.into(),
                        pwr_proportional_gain: pwr.pwr_proportional_gain,
                        pwr_pstate_related_k: -F32::from(max_ps_scaled) / pwr.max_power_mw.into(),
                        pwr_pstate_max_dc_offset: pwr.pwr_min_duty_cycle as i32
                            - max_ps_scaled as i32,
                        max_pstate_scaled_2: max_ps_scaled,
                        max_power_2: pwr.max_power_mw,
                        max_pstate_scaled_3: max_ps_scaled,
                        ppm_filter_tc_periods_x4: ppm_filter_tc_periods * 4,
                        ppm_filter_a_neg: f32!(1.0) - ppm_filter_a,
                        ppm_filter_a: ppm_filter_a,
                        ppm_ki_dt: pwr.ppm_ki * period_s,
                        #[ver(V >= V13_0B4)]
                        unk_6fc: f32!(65536.0),
                        #[ver(V < V13_0B4)]
                        unk_6fc: if self.cfg.chip_id != 0x8103 {
                            f32!(65536.0)
                        } else {
                            f32!(0.0)
                        },
                        ppm_kp: pwr.ppm_kp,
                        pwr_min_duty_cycle: pwr.pwr_min_duty_cycle,
                        max_pstate_scaled_4: max_ps_scaled,
                        unk_71c: f32!(0.0),
                        max_power_3: pwr.max_power_mw,
                        cur_power_mw_2: 0x0,
                        ppm_filter_tc_ms: pwr.ppm_filter_time_constant_ms,
                        #[ver(V >= V13_0B4)]
                        unk_730_0: 0x232800,
                        perf_tgt_utilization: pwr.perf_tgt_utilization,
                        perf_boost_min_util: pwr.perf_boost_min_util,
                        perf_boost_ce_step: pwr.perf_boost_ce_step,
                        perf_reset_iters: pwr.perf_reset_iters,
                        unk_774: 6,
                        unk_778: 1,
                        perf_filter_drop_threshold: pwr.perf_filter_drop_threshold,
                        perf_filter_a_neg: f32!(1.0) - perf_filter_a,
                        perf_filter_a2_neg: f32!(1.0) - perf_filter_a2,
                        perf_filter_a: perf_filter_a,
                        perf_filter_a2: perf_filter_a2,
                        perf_ki: pwr.perf_integral_gain,
                        perf_ki2: pwr.perf_integral_gain2,
                        perf_integral_min_clamp: pwr.perf_integral_min_clamp.into(),
                        unk_79c: f32!(95.0),
                        perf_kp: pwr.perf_proportional_gain,
                        perf_kp2: pwr.perf_proportional_gain2,
                        boost_state_unk_k: F32::from(boost_ps_count) / f32!(0.95),
                        base_pstate_scaled_2: base_ps_scaled,
                        max_pstate_scaled_5: max_ps_scaled,
                        base_pstate_scaled_3: base_ps_scaled,
                        perf_tgt_utilization_2: pwr.perf_tgt_utilization,
                        base_pstate_scaled_4: base_ps_scaled,
                        unk_7fc: f32!(65536.0),
                        pwr_min_duty_cycle_2: pwr.pwr_min_duty_cycle.into(),
                        max_pstate_scaled_6: max_ps_scaled.into(),
                        max_freq_mhz: pwr.max_freq_mhz,
                        pwr_min_duty_cycle_3: pwr.pwr_min_duty_cycle,
                        min_pstate_scaled_4: f32!(100.0),
                        max_pstate_scaled_7: max_ps_scaled,
                        unk_alpha_neg: f32!(0.8),
                        unk_alpha: f32!(0.2),
                        fast_die0_sensor_mask: U64(self.cfg.fast_die0_sensor_mask),
                        fast_die0_release_temp_cc: 100 * pwr.fast_die0_release_temp,
                        unk_87c: self.cfg.da.unk_87c,
                        unk_880: 0x4,
                        unk_894: f32!(1.0),

                        fast_die0_ki_dt: pwr.fast_die0_integral_gain * period_s,
                        unk_8a8: f32!(65536.0),
                        fast_die0_kp: pwr.fast_die0_proportional_gain,
                        pwr_min_duty_cycle_4: pwr.pwr_min_duty_cycle,
                        max_pstate_scaled_8: max_ps_scaled,
                        max_pstate_scaled_9: max_ps_scaled,
                        fast_die0_prop_tgt_delta: 100 * pwr.fast_die0_prop_tgt_delta,
                        unk_8cc: self.cfg.da.unk_8cc,
                        max_pstate_scaled_10: max_ps_scaled,
                        max_pstate_scaled_11: max_ps_scaled,
                        unk_c2c: 1,
                        power_zone_count: pwr.power_zones.len() as u32,
                        max_power_4: pwr.max_power_mw,
                        max_power_5: pwr.max_power_mw,
                        max_power_6: pwr.max_power_mw,
                        avg_power_target_filter_a_neg: f32!(1.0) - avg_power_target_filter_a,
                        avg_power_target_filter_a: avg_power_target_filter_a,
                        avg_power_target_filter_tc_x4: 4 * pwr.avg_power_target_filter_tc,
                        avg_power_target_filter_tc_xperiod: period_ms
                            * pwr.avg_power_target_filter_tc,
                        #[ver(V >= V13_0B4)]
                        base_clock_mhz: base_clock_mhz,
                        avg_power_filter_tc_periods_x4: 4 * avg_power_filter_tc_periods,
                        avg_power_filter_a_neg: f32!(1.0) - avg_power_filter_a,
                        avg_power_filter_a: avg_power_filter_a,
                        avg_power_ki_dt: pwr.avg_power_ki_only * period_s,
                        unk_d20: f32!(65536.0),
                        avg_power_kp: pwr.avg_power_kp,
                        avg_power_min_duty_cycle: pwr.avg_power_min_duty_cycle,
                        max_pstate_scaled_12: max_ps_scaled,
                        max_pstate_scaled_13: max_ps_scaled,
                        max_power_7: pwr.max_power_mw.into(),
                        max_power_8: pwr.max_power_mw,
                        unk_d4c: pwr.avg_power_filter_tc_ms,
                        #[ver(V >= V13_0B4)]
                        base_clock_mhz_2: base_clock_mhz,
                        max_pstate_scaled_14: max_ps_scaled,
                        t81xx_data: match self.cfg.chip_id {
                            0x8103 | 0x8112 => Self::t81xx_data(self.dyncfg),
                            _ => Default::default(),
                        },
                        #[ver(V >= V13_0B4)]
                        unk_e10_0: raw::HwDataA130Extra {
                            unk_38: 4,
                            unk_3c: 8000,
                            unk_40: 2500,
                            unk_48: 0xffffffff,
                            unk_4c: 50,
                            unk_54: 50,
                            unk_58: 0x1,
                            unk_60: f32!(0.88888888),
                            unk_64: f32!(0.66666666),
                            unk_68: f32!(0.111111111),
                            unk_6c: f32!(0.33333333),
                            unk_70: f32!(-0.4),
                            unk_74: f32!(-0.8),
                            unk_7c: f32!(65536.0),
                            unk_80: f32!(-5.0),
                            unk_84: f32!(-10.0),
                            unk_8c: 40,
                            unk_90: 600,
                            unk_9c: f32!(8000.0),
                            unk_a0: 1400,
                            unk_a8: 72,
                            unk_ac: 24,
                            unk_b0: 1728000,
                            unk_b8: 576000,
                            unk_c4: f32!(65536.0),
                            unk_114: f32!(65536.0),
                            unk_124: 40,
                            unk_128: 600,
                            ..Default::default()
                        },
                        fast_die0_sensor_mask_2: U64(self.cfg.fast_die0_sensor_mask),
                        unk_e24: self.cfg.da.unk_e24,
                        unk_e28: 1,
                        fast_die0_sensor_mask_alt: U64(self.cfg.fast_die0_sensor_mask_alt),
                        fast_die0_sensor_present: self.cfg.fast_die0_sensor_present,
                        #[ver(V < V13_0B4)]
                        unk_1638: Array::new([0, 0, 0, 0, 1, 0, 0, 0]),
                        hws1: Self::hw_shared1(self.cfg),
                        hws2: *Self::hw_shared2(self.cfg)?,
                        hws3: *Self::hw_shared3(self.cfg)?,
                        unk_3ce8: 1,
                        ..Default::default()
                    }
                );

                for i in 0..self.dyncfg.pwr.perf_states.len() {
                    raw.sram_k[i] = self.cfg.sram_k;
                }

                for (i, coef) in pwr.core_leak_coef.iter().enumerate() {
                    raw.core_leak_coef[i] = *coef;
                }

                for (i, coef) in pwr.sram_leak_coef.iter().enumerate() {
                    raw.sram_leak_coef[i] = *coef;
                }

                for i in 0..self.dyncfg.id.num_clusters as usize {
                    if let Some(coef_a) = self.cfg.unk_coef_a.get(i) {
                        (*raw.unk_coef_a1[i])[..coef_a.len()].copy_from_slice(coef_a);
                        (*raw.unk_coef_a2[i])[..coef_a.len()].copy_from_slice(coef_a);
                    }
                    if let Some(coef_b) = self.cfg.unk_coef_b.get(i) {
                        (*raw.unk_coef_b1[i])[..coef_b.len()].copy_from_slice(coef_b);
                        (*raw.unk_coef_b2[i])[..coef_b.len()].copy_from_slice(coef_b);
                    }
                }

                for (i, pz) in pwr.power_zones.iter().enumerate() {
                    raw.power_zones[i].target = pz.target;
                    raw.power_zones[i].target_off = pz.target - pz.target_offset;
                    raw.power_zones[i].filter_tc_x4 = 4 * pz.filter_tc;
                    raw.power_zones[i].filter_tc_xperiod = period_ms * pz.filter_tc;
                    let filter_a = f32!(1.0) / pz.filter_tc.into();
                    raw.power_zones[i].filter_a = filter_a;
                    raw.power_zones[i].filter_a_neg = f32!(1.0) - filter_a;
                    #[ver(V >= V13_0B4)]
                    raw.power_zones[i].unk_10 = 1320000000;
                }

                Ok(raw)
            })
    }

    #[inline(never)]
    fn hwdata_b(&mut self) -> Result<GpuObject<HwDataB::ver>> {
        self.alloc
            .private
            .new_inplace(Default::default(), |_inner, ptr| {
                let raw = place!(
                    ptr,
                    raw::HwDataB::ver {
                        // Userspace VA map related
                        #[ver(V < V13_0B4)]
                        unk_0: 0x13_00000000,
                        unk_8: 0x14_00000000,
                        #[ver(V < V13_0B4)]
                        unk_10: 0x1_00000000,
                        unk_18: 0xffc00000,
                        unk_20: 0x11_00000000,
                        unk_28: 0x11_00000000,
                        // userspace address?
                        unk_30: 0x6f_ffff8000,
                        // unmapped?
                        unkptr_38: 0xffffffa0_11800000,
                        // TODO: yuv matrices
                        chip_id: self.cfg.chip_id,
                        unk_454: 0x1,
                        unk_458: 0x1,
                        unk_460: 0x1,
                        unk_464: 0x1,
                        unk_468: 0x1,
                        unk_47c: 0x1,
                        unk_484: 0x1,
                        unk_48c: 0x1,
                        base_clock_khz: self.cfg.base_clock_hz / 1000,
                        power_sample_period: self.dyncfg.pwr.power_sample_period,
                        unk_49c: 0x1,
                        unk_4a0: 0x1,
                        unk_4a4: 0x1,
                        unk_4c0: 0x1f,
                        unk_4e0: self.cfg.db.unk_4e0,
                        unk_4f0: 0x1,
                        unk_4f4: 0x1,
                        unk_504: 0x31,
                        unk_524: 0x1, // use_secure_cache_flush
                        unk_534: self.cfg.db.unk_534,
                        num_frags: self.dyncfg.id.num_frags * self.dyncfg.id.num_clusters,
                        unk_554: 0x1,
                        uat_ttb_base: self.dyncfg.uat_ttb_base,
                        gpu_core_id: self.cfg.gpu_core as u32,
                        gpu_rev_id: self.dyncfg.id.gpu_rev_id as u32,
                        num_cores: self.dyncfg.id.num_cores * self.dyncfg.id.num_clusters,
                        max_pstate: self.dyncfg.pwr.perf_states.len() as u32 - 1,
                        #[ver(V < V13_0B4)]
                        num_pstates: self.dyncfg.pwr.perf_states.len() as u32,
                        #[ver(V < V13_0B4)]
                        min_sram_volt: self.dyncfg.pwr.min_sram_microvolt / 1000,
                        #[ver(V < V13_0B4)]
                        unk_ab8: self.cfg.db.unk_ab8,
                        #[ver(V < V13_0B4)]
                        unk_abc: self.cfg.db.unk_abc,
                        #[ver(V < V13_0B4)]
                        unk_ac0: 0x1020,

                        #[ver(V >= V13_0B4)]
                        unk_ae4: Array::new([0x0, 0x3, 0x7, 0x7]),
                        #[ver(V < V13_0B4)]
                        unk_ae4: Array::new([0x0, 0xf, 0x3f, 0x3f]),
                        unk_b10: 0x1,
                        unk_b24: 0x1,
                        unk_b28: 0x1,
                        unk_b2c: 0x1,
                        unk_b30: self.cfg.db.unk_b30,
                        #[ver(V >= V13_0B4)]
                        unk_b38_0: 1,
                        #[ver(V >= V13_0B4)]
                        unk_b38_4: 1,
                        unk_b38: Array::new([0xffffffff; 12]),
                        #[ver(V >= V13_0B4)]
                        unk_c3c: 0x19,
                        ..Default::default()
                    }
                );

                let base_ps = self.dyncfg.pwr.perf_base_pstate as usize;
                let max_ps = self.dyncfg.pwr.perf_max_pstate as usize;
                let base_freq = self.dyncfg.pwr.perf_states[base_ps].freq_hz;
                let max_freq = self.dyncfg.pwr.perf_states[max_ps].freq_hz;

                for (i, ps) in self.dyncfg.pwr.perf_states.iter().enumerate() {
                    raw.frequencies[i] = ps.freq_hz / 1000000;
                    for (j, mv) in ps.volt_mv.iter().enumerate() {
                        let sram_mv = (*mv).max(self.dyncfg.pwr.min_sram_microvolt / 1000);
                        raw.voltages[i][j] = *mv;
                        raw.voltages_sram[i][j] = sram_mv;
                    }
                    raw.sram_k[i] = self.cfg.sram_k;
                    raw.rel_max_powers[i] = ps.pwr_mw * 100 / self.dyncfg.pwr.max_power_mw;
                    raw.rel_boost_freqs[i] = if i > base_ps {
                        (ps.freq_hz - base_freq) / ((max_freq - base_freq) / 100)
                    } else {
                        0
                    };
                }

                Ok(raw)
            })
    }

    #[inline(never)]
    fn globals(&mut self) -> Result<GpuObject<Globals::ver>> {
        self.alloc
            .shared
            .new_inplace(Default::default(), |_inner, ptr| {
                let pwr = &self.dyncfg.pwr;
                let period_ms = pwr.power_sample_period;
                let period_s = F32::from(period_ms) / f32!(1000.0);
                let avg_power_filter_tc_periods = pwr.avg_power_filter_tc_ms / period_ms;

                let max_ps = pwr.perf_max_pstate;
                let max_ps_scaled = 100 * max_ps;

                let raw = place!(
                    ptr,
                    raw::Globals::ver {
                        //ktrace_enable: 0xffffffff,
                        ktrace_enable: 0,
                        #[ver(V >= V13_0B4)]
                        unk_28_0: 0, // debug
                        unk_28: 1,
                        #[ver(V >= V13_0B4)]
                        unk_2c_0: 0,
                        unk_2c: 1,
                        // 0 on G13G >=13.0b4 and G13X all versions?
                        unk_30: 0,
                        unk_34: 120,
                        sub: raw::GlobalsSub::ver {
                            unk_54: 0xffff,
                            unk_56: 40,
                            unk_58: 0xffff,
                            unk_5e: 1,
                            unk_66: 1,
                            ..Default::default()
                        },
                        unk_8900: 1,
                        max_power: pwr.max_power_mw,
                        max_pstate_scaled: max_ps_scaled,
                        max_pstate_scaled_2: max_ps_scaled,
                        max_pstate_scaled_3: max_ps_scaled,
                        power_zone_count: pwr.power_zones.len() as u32,
                        avg_power_filter_tc_periods: avg_power_filter_tc_periods,
                        avg_power_ki_dt: pwr.avg_power_ki_only * period_s,
                        avg_power_kp: pwr.avg_power_kp,
                        avg_power_min_duty_cycle: pwr.avg_power_min_duty_cycle,
                        avg_power_target_filter_tc: pwr.avg_power_target_filter_tc,
                        unk_89bc: self.cfg.da.unk_8cc,
                        fast_die0_release_temp: 100 * pwr.fast_die0_release_temp,
                        unk_89c4: self.cfg.da.unk_87c,
                        fast_die0_prop_tgt_delta: 100 * pwr.fast_die0_prop_tgt_delta,
                        fast_die0_kp: pwr.fast_die0_proportional_gain,
                        fast_die0_ki_dt: pwr.fast_die0_integral_gain * period_s,
                        unk_89e0: 1,
                        max_power_2: pwr.max_power_mw,
                        ppm_kp: pwr.ppm_kp,
                        ppm_ki_dt: pwr.ppm_ki * period_s,
                        #[ver(V >= V13_0B4)]
                        unk_89f4_8: 1,
                        hws1: Self::hw_shared1(self.cfg),
                        hws2: *Self::hw_shared2(self.cfg)?,
                        hws3: *Self::hw_shared3(self.cfg)?,
                        unk_900c: 1,
                        #[ver(V >= V13_0B4)]
                        unk_9010_0: 1,
                        #[ver(V >= V13_0B4)]
                        unk_903c: 1,
                        #[ver(V < V13_0B4)]
                        unk_903c: 0,
                        fault_control: *crate::fault_control.read(),
                        do_init: 1,
                        unk_11020: 40,
                        unk_11024: 10,
                        unk_11028: 250,
                        #[ver(V >= V13_0B4)]
                        unk_1102c_0: 1,
                        #[ver(V >= V13_0B4)]
                        unk_1102c_4: 1,
                        #[ver(V >= V13_0B4)]
                        unk_1102c_8: 100,
                        #[ver(V >= V13_0B4)]
                        unk_1102c_c: 1,
                        idle_off_delay_ms: AtomicU32::new(pwr.idle_off_delay_ms),
                        fender_idle_off_delay_ms: pwr.fender_idle_off_delay_ms,
                        fw_early_wake_timeout_ms: pwr.fw_early_wake_timeout_ms,
                        unk_118e0: 40,
                        #[ver(V >= V13_0B4)]
                        unk_118e4_0: 50,
                        #[ver(V >= V13_0B4)]
                        unk_11edc: 8,
                        #[ver(V >= V13_0B4)]
                        unk_11efc: 8,
                        ..Default::default()
                    }
                );

                for (i, pz) in pwr.power_zones.iter().enumerate() {
                    raw.power_zones[i].target = pz.target;
                    raw.power_zones[i].target_off = pz.target - pz.target_offset;
                    raw.power_zones[i].filter_tc = pz.filter_tc;
                }

                if let Some(tab) = self.cfg.global_tab.as_ref() {
                    for (i, x) in tab.iter().enumerate() {
                        raw.unk_118ec[i] = *x;
                    }
                    raw.unk_118e8 = 1;
                }

                Ok(raw)
            })
    }

    #[inline(never)]
    fn make_channel<T: GpuStruct + Debug + Default, U: Copy + Default>(
        &mut self,
        count: usize,
        rx: bool,
    ) -> Result<ChannelRing<T, U>>
    where
        for<'b> <T as GpuStruct>::Raw<'b>: Default + Debug,
    {
        let ring_alloc = if rx {
            &mut self.alloc.shared
        } else {
            &mut self.alloc.private
        };

        let ring = ring_alloc.array_empty(count)?;

        Ok(ChannelRing {
            state: self
                .alloc
                .shared
                .new_object(Default::default(), |_inner| Default::default())?,
            ring,
        })
    }

    #[inline(never)]
    fn runtime_pointers(&mut self) -> Result<GpuObject<RuntimePointers::ver>> {
        let hwa = self.hwdata_a()?;
        let hwb = self.hwdata_b()?;

        let pointers: Box<RuntimePointers::ver> = box_in_place!(RuntimePointers::ver {
            stats: Stats::ver {
                vtx: self.alloc.private.new_default::<GpuGlobalStatsVtx::ver>()?,
                frag: self.alloc.private.new_inplace(
                    Default::default(),
                    |_inner, ptr: *mut MaybeUninit<raw::GpuGlobalStatsFrag::ver>| {
                        Ok(place!(
                            ptr,
                            raw::GpuGlobalStatsFrag::ver {
                                stats: raw::GpuStatsFrag::ver {
                                    cur_stamp_id: -1,
                                    unk_118: -1,
                                    ..Default::default()
                                },
                                ..Default::default()
                            }
                        ))
                    },
                )?,
                comp: self.alloc.private.array_empty(0x980)?,
            },

            hwdata_a: hwa,
            unkptr_190: self.alloc.private.array_empty(0x80)?,
            unkptr_198: self.alloc.private.array_empty(0xc0)?,
            hwdata_b: hwb,

            unkptr_1b8: self.alloc.private.array_empty(0x1000)?,
            unkptr_1c0: self.alloc.private.array_empty(0x300)?,
            unkptr_1c8: self.alloc.private.array_empty(0x1000)?,

            buffer_mgr_ctl: self.alloc.gpu.array_empty(127)?,
        })?;

        self.alloc.private.new_boxed(pointers, |inner, ptr| {
            Ok(place!(
                ptr,
                raw::RuntimePointers::ver {
                    pipes: Default::default(),
                    device_control: Default::default(),
                    event: Default::default(),
                    fw_log: Default::default(),
                    ktrace: Default::default(),
                    stats: Default::default(),

                    stats_vtx: inner.stats.vtx.gpu_pointer(),
                    stats_frag: inner.stats.frag.gpu_pointer(),
                    stats_comp: inner.stats.comp.gpu_pointer(),

                    hwdata_a: inner.hwdata_a.gpu_pointer(),
                    unkptr_190: inner.unkptr_190.gpu_pointer(),
                    unkptr_198: inner.unkptr_198.gpu_pointer(),
                    hwdata_b: inner.hwdata_b.gpu_pointer(),
                    hwdata_b_2: inner.hwdata_b.gpu_pointer(),

                    fwlog_buf: None,

                    unkptr_1b8: inner.unkptr_1b8.gpu_pointer(),
                    unkptr_1c0: inner.unkptr_1c0.gpu_pointer(),
                    unkptr_1c8: inner.unkptr_1c8.gpu_pointer(),

                    buffer_mgr_ctl: inner.buffer_mgr_ctl.gpu_pointer(),
                    buffer_mgr_ctl_2: inner.buffer_mgr_ctl.gpu_pointer(),

                    __pad0: Default::default(),
                    unk_160: 0,
                    unk_168: 0,
                    unk_1d0: 0,
                    unk_1d4: 0,
                    unk_1d8: Default::default(),

                    __pad1: Default::default(),
                    gpu_scratch: raw::RuntimeScratch {
                        unk_6b38: 0xff,
                        ..Default::default()
                    },
                }
            ))
        })
    }

    #[inline(never)]
    fn fw_status(&mut self) -> Result<GpuObject<FwStatus>> {
        self.alloc
            .shared
            .new_object(Default::default(), |_inner| Default::default())
    }

    #[inline(never)]
    fn uat_level_info(
        cfg: &hw::HwConfig,
        index_shift: usize,
        num_entries: usize,
    ) -> raw::UatLevelInfo {
        raw::UatLevelInfo {
            index_shift: index_shift as _,
            unk_1: 14,
            unk_2: 14,
            unk_3: 8,
            unk_4: 0x4000,
            num_entries: num_entries as _,
            unk_8: 1,
            unk_10: ((1u64 << cfg.uat_oas) - 1) & !(mmu::UAT_PGMSK as u64),
            index_mask: ((num_entries - 1) << index_shift) as u64,
        }
    }

    #[inline(never)]
    pub(crate) fn build(&mut self) -> Result<Box<GpuObject<InitData::ver>>> {
        let inner: Box<InitData::ver> = box_in_place!(InitData::ver {
            unk_buf: self.alloc.shared_ro.array_empty(0x4000)?,
            runtime_pointers: self.runtime_pointers()?,
            globals: self.globals()?,
            fw_status: self.fw_status()?,
        })?;

        Ok(Box::try_new(self.alloc.shared_ro.new_boxed(
            inner,
            |inner, ptr| {
                Ok(place!(
                    ptr,
                    raw::InitData::ver {
                        #[ver(V >= V13_0B4)]
                        ver_info: Array::new([1, 1, 16, 1]),
                        unk_buf: inner.unk_buf.gpu_pointer(),
                        unk_8: 0,
                        unk_c: 0,
                        runtime_pointers: inner.runtime_pointers.gpu_pointer(),
                        globals: inner.globals.gpu_pointer(),
                        fw_status: inner.fw_status.gpu_pointer(),
                        uat_page_size: 0x4000,
                        uat_page_bits: 14,
                        uat_num_levels: 3,
                        uat_level_info: Array::new([
                            Self::uat_level_info(self.cfg, 36, 8),
                            Self::uat_level_info(self.cfg, 25, 2048),
                            Self::uat_level_info(self.cfg, 14, 2048),
                        ]),
                        __pad0: Default::default(),
                        host_mapped_fw_allocations: 1,
                        unk_ac: 0,
                        unk_b0: 0,
                        unk_b4: 0,
                        unk_b8: 0,
                    }
                ))
            },
        )?)?)
    }
}
