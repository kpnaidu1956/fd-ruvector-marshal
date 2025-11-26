//! # Kuiper Belt Object Data
//!
//! Pre-loaded Trans-Neptunian Object (TNO) data from NASA/JPL Small-Body Database.
//! Data source: https://ssd-api.jpl.nasa.gov/sbdb_query.api
//!
//! Fields: name, a (AU), e, i (deg), q (AU), ad (AU), period (days), omega (deg), w (deg), H, class

use super::kuiper_cluster::KuiperBeltObject;

/// Returns a comprehensive dataset of known Kuiper Belt Objects
/// Data sourced from NASA/JPL SBDB Query API
pub fn get_kbo_data() -> Vec<KuiperBeltObject> {
    vec![
        // ═══════════════════════════════════════════════════════════════
        // DWARF PLANETS AND MAJOR TNOs
        // ═══════════════════════════════════════════════════════════════
        KuiperBeltObject {
            name: "134340 Pluto".to_string(),
            a: 39.59, e: 0.2518, i: 17.15, q: 29.619, ad: 49.56,
            period: 91000.0, omega: 110.29, w: 113.71, h: Some(-0.54), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "136199 Eris".to_string(),
            a: 68.0, e: 0.4370, i: 43.87, q: 38.284, ad: 97.71,
            period: 205000.0, omega: 36.03, w: 150.73, h: Some(-1.25), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "136108 Haumea".to_string(),
            a: 43.01, e: 0.1958, i: 28.21, q: 34.586, ad: 51.42,
            period: 103000.0, omega: 121.80, w: 240.89, h: Some(0.14), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "136472 Makemake".to_string(),
            a: 45.51, e: 0.1604, i: 29.03, q: 38.210, ad: 52.81,
            period: 112000.0, omega: 79.27, w: 297.08, h: Some(-0.25), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "225088 Gonggong".to_string(),
            a: 66.89, e: 0.5032, i: 30.87, q: 33.235, ad: 100.55,
            period: 200000.0, omega: 336.84, w: 206.64, h: Some(1.84), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "90377 Sedna".to_string(),
            a: 549.5, e: 0.8613, i: 11.93, q: 76.223, ad: 1022.86,
            period: 4710000.0, omega: 144.48, w: 311.01, h: Some(1.49), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "50000 Quaoar".to_string(),
            a: 43.15, e: 0.0358, i: 7.99, q: 41.601, ad: 44.69,
            period: 104000.0, omega: 188.96, w: 163.92, h: Some(2.41), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "90482 Orcus".to_string(),
            a: 39.34, e: 0.2217, i: 20.56, q: 30.614, ad: 48.06,
            period: 90100.0, omega: 268.39, w: 73.72, h: Some(2.14), class: "TNO".to_string(),
        },

        // ═══════════════════════════════════════════════════════════════
        // PLUTINOS (3:2 Neptune Resonance ~39.4 AU)
        // ═══════════════════════════════════════════════════════════════
        KuiperBeltObject {
            name: "15810 Arawn".to_string(),
            a: 39.21, e: 0.1141, i: 3.81, q: 34.734, ad: 43.68,
            period: 89700.0, omega: 144.74, w: 101.22, h: Some(7.68), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "28978 Ixion".to_string(),
            a: 39.35, e: 0.2442, i: 19.67, q: 29.740, ad: 48.96,
            period: 90200.0, omega: 71.09, w: 300.66, h: Some(3.47), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "38628 Huya".to_string(),
            a: 39.21, e: 0.2729, i: 15.48, q: 28.513, ad: 49.91,
            period: 89700.0, omega: 169.31, w: 67.51, h: Some(4.79), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "47171 Lempo".to_string(),
            a: 39.72, e: 0.2298, i: 8.40, q: 30.591, ad: 48.85,
            period: 91400.0, omega: 97.17, w: 295.82, h: Some(4.93), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "208996 Achlys".to_string(),
            a: 39.63, e: 0.1748, i: 13.55, q: 32.699, ad: 46.56,
            period: 91100.0, omega: 251.87, w: 14.40, h: Some(3.72), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "84922 (2003 VS2)".to_string(),
            a: 39.71, e: 0.0816, i: 14.76, q: 36.476, ad: 42.95,
            period: 91400.0, omega: 302.78, w: 115.15, h: Some(3.99), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "455502 (2003 UZ413)".to_string(),
            a: 39.43, e: 0.2182, i: 12.04, q: 30.824, ad: 48.03,
            period: 90400.0, omega: 136.13, w: 146.24, h: Some(4.27), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "15788 (1993 SB)".to_string(),
            a: 39.73, e: 0.3267, i: 1.94, q: 26.754, ad: 52.71,
            period: 91500.0, omega: 354.90, w: 79.31, h: Some(7.96), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "15789 (1993 SC)".to_string(),
            a: 39.74, e: 0.1839, i: 5.16, q: 32.433, ad: 47.05,
            period: 91500.0, omega: 354.69, w: 318.64, h: Some(7.09), class: "TNO".to_string(),
        },

        // ═══════════════════════════════════════════════════════════════
        // CLASSICAL KUIPER BELT (Cubewanos, 42-48 AU, low e)
        // ═══════════════════════════════════════════════════════════════
        KuiperBeltObject {
            name: "15760 Albion".to_string(),
            a: 44.2, e: 0.0725, i: 2.19, q: 40.995, ad: 47.40,
            period: 107000.0, omega: 359.47, w: 6.89, h: Some(7.18), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "20000 Varuna".to_string(),
            a: 43.18, e: 0.0525, i: 17.14, q: 40.909, ad: 45.45,
            period: 104000.0, omega: 97.21, w: 273.22, h: Some(3.79), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "19521 Chaos".to_string(),
            a: 46.11, e: 0.1105, i: 12.02, q: 41.013, ad: 51.20,
            period: 114000.0, omega: 49.91, w: 56.61, h: Some(4.63), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "79360 Sila-Nunam".to_string(),
            a: 44.04, e: 0.0141, i: 2.24, q: 43.415, ad: 44.66,
            period: 107000.0, omega: 304.26, w: 214.87, h: Some(5.26), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "66652 Borasisi".to_string(),
            a: 43.79, e: 0.0849, i: 0.56, q: 40.075, ad: 47.51,
            period: 106000.0, omega: 84.65, w: 198.96, h: Some(5.86), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "58534 Logos".to_string(),
            a: 45.23, e: 0.1227, i: 2.90, q: 39.681, ad: 50.79,
            period: 111000.0, omega: 132.51, w: 336.06, h: Some(6.87), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "88611 Teharonhiawako".to_string(),
            a: 44.05, e: 0.0266, i: 2.58, q: 42.877, ad: 45.22,
            period: 107000.0, omega: 304.87, w: 232.38, h: Some(5.98), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "53311 Deucalion".to_string(),
            a: 43.89, e: 0.0588, i: 0.37, q: 41.305, ad: 46.47,
            period: 106000.0, omega: 51.30, w: 244.04, h: Some(6.71), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "120347 Salacia".to_string(),
            a: 42.11, e: 0.1034, i: 23.93, q: 37.761, ad: 46.47,
            period: 99800.0, omega: 280.26, w: 309.48, h: Some(4.12), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "145452 Ritona".to_string(),
            a: 41.55, e: 0.0239, i: 19.26, q: 40.561, ad: 42.55,
            period: 97800.0, omega: 187.00, w: 178.79, h: Some(3.69), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "307261 Mani".to_string(),
            a: 41.6, e: 0.1487, i: 17.70, q: 35.410, ad: 47.78,
            period: 98000.0, omega: 216.19, w: 215.22, h: Some(3.64), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "55565 Aya".to_string(),
            a: 47.3, e: 0.1277, i: 24.34, q: 41.259, ad: 53.34,
            period: 119000.0, omega: 297.37, w: 294.59, h: Some(3.44), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "174567 Varda".to_string(),
            a: 45.54, e: 0.1430, i: 21.51, q: 39.026, ad: 52.05,
            period: 112000.0, omega: 184.12, w: 184.97, h: Some(3.46), class: "TNO".to_string(),
        },

        // ═══════════════════════════════════════════════════════════════
        // TWOTINOS (2:1 Neptune Resonance ~47.8 AU)
        // ═══════════════════════════════════════════════════════════════
        KuiperBeltObject {
            name: "20161 (1996 TR66)".to_string(),
            a: 47.98, e: 0.3957, i: 12.43, q: 28.993, ad: 66.97,
            period: 121000.0, omega: 343.11, w: 309.94, h: Some(7.43), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "119979 (2002 WC19)".to_string(),
            a: 48.28, e: 0.2662, i: 9.16, q: 35.427, ad: 61.14,
            period: 123000.0, omega: 109.74, w: 43.25, h: Some(4.66), class: "TNO".to_string(),
        },

        // ═══════════════════════════════════════════════════════════════
        // SCATTERED DISK OBJECTS
        // ═══════════════════════════════════════════════════════════════
        KuiperBeltObject {
            name: "15874 (1996 TL66)".to_string(),
            a: 84.89, e: 0.5866, i: 23.96, q: 35.094, ad: 134.69,
            period: 286000.0, omega: 217.70, w: 185.14, h: Some(5.41), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "26181 (1996 GQ21)".to_string(),
            a: 92.48, e: 0.5874, i: 13.36, q: 38.152, ad: 146.81,
            period: 325000.0, omega: 194.22, w: 356.02, h: Some(4.84), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "26375 (1999 DE9)".to_string(),
            a: 55.5, e: 0.4201, i: 7.61, q: 32.184, ad: 78.81,
            period: 151000.0, omega: 322.88, w: 159.37, h: Some(4.89), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "82075 (2000 YW134)".to_string(),
            a: 58.23, e: 0.2936, i: 19.77, q: 41.128, ad: 75.32,
            period: 162000.0, omega: 126.91, w: 316.59, h: Some(4.65), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "84522 (2002 TC302)".to_string(),
            a: 55.84, e: 0.2995, i: 35.01, q: 39.113, ad: 72.56,
            period: 152000.0, omega: 23.83, w: 86.07, h: Some(3.92), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "145480 (2005 TB190)".to_string(),
            a: 75.93, e: 0.3912, i: 26.48, q: 46.227, ad: 105.64,
            period: 242000.0, omega: 180.46, w: 171.99, h: Some(4.49), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "229762 G!kun||'homdima".to_string(),
            a: 74.59, e: 0.4961, i: 23.33, q: 37.585, ad: 111.59,
            period: 235000.0, omega: 131.24, w: 345.94, h: Some(3.45), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "145451 Rumina".to_string(),
            a: 92.27, e: 0.6190, i: 28.70, q: 35.160, ad: 149.39,
            period: 324000.0, omega: 84.63, w: 318.73, h: Some(4.59), class: "TNO".to_string(),
        },

        // ═══════════════════════════════════════════════════════════════
        // EXTREME/DETACHED OBJECTS
        // ═══════════════════════════════════════════════════════════════
        KuiperBeltObject {
            name: "148209 (2000 CR105)".to_string(),
            a: 228.7, e: 0.8071, i: 22.71, q: 44.117, ad: 413.29,
            period: 1260000.0, omega: 128.21, w: 316.92, h: Some(6.14), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "82158 (2001 FP185)".to_string(),
            a: 213.4, e: 0.8398, i: 30.80, q: 34.190, ad: 392.66,
            period: 1140000.0, omega: 179.36, w: 6.62, h: Some(6.16), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "87269 (2000 OO67)".to_string(),
            a: 617.9, e: 0.9663, i: 20.05, q: 20.850, ad: 1215.04,
            period: 5610000.0, omega: 142.38, w: 212.72, h: Some(9.10), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "308933 (2006 SQ372)".to_string(),
            a: 839.3, e: 0.9711, i: 19.46, q: 24.226, ad: 1654.33,
            period: 8880000.0, omega: 197.37, w: 122.65, h: Some(7.94), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "445473 (2010 VZ98)".to_string(),
            a: 159.8, e: 0.7851, i: 4.51, q: 34.356, ad: 285.32,
            period: 738000.0, omega: 117.44, w: 313.74, h: Some(5.04), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "303775 (2005 QU182)".to_string(),
            a: 112.2, e: 0.6696, i: 14.01, q: 37.059, ad: 187.28,
            period: 434000.0, omega: 78.54, w: 224.26, h: Some(3.74), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "437360 (2013 TV158)".to_string(),
            a: 114.1, e: 0.6801, i: 31.14, q: 36.482, ad: 191.62,
            period: 445000.0, omega: 181.07, w: 232.30, h: Some(6.38), class: "TNO".to_string(),
        },

        // ═══════════════════════════════════════════════════════════════
        // HIGH INCLINATION OBJECTS
        // ═══════════════════════════════════════════════════════════════
        KuiperBeltObject {
            name: "65407 (2002 RP120)".to_string(),
            a: 54.53, e: 0.9542, i: 119.37, q: 2.498, ad: 106.57,
            period: 147000.0, omega: 39.01, w: 357.97, h: Some(12.43), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "127546 (2002 XU93)".to_string(),
            a: 66.9, e: 0.6862, i: 77.95, q: 20.991, ad: 112.80,
            period: 200000.0, omega: 90.21, w: 28.02, h: Some(8.06), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "336756 (2010 NV1)".to_string(),
            a: 305.2, e: 0.9690, i: 140.82, q: 9.457, ad: 600.93,
            period: 1950000.0, omega: 136.32, w: 133.20, h: Some(10.55), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "418993 (2009 MS9)".to_string(),
            a: 375.7, e: 0.9706, i: 67.96, q: 11.046, ad: 740.43,
            period: 2660000.0, omega: 220.18, w: 128.90, h: Some(9.74), class: "TNO".to_string(),
        },

        // ═══════════════════════════════════════════════════════════════
        // 5:2 RESONANCE (~55.4 AU)
        // ═══════════════════════════════════════════════════════════════
        KuiperBeltObject {
            name: "84719 (2002 VR128)".to_string(),
            a: 39.65, e: 0.2618, i: 14.00, q: 29.272, ad: 50.04,
            period: 91200.0, omega: 23.05, w: 289.58, h: Some(5.19), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "120132 (2003 FY128)".to_string(),
            a: 49.28, e: 0.2508, i: 11.77, q: 36.925, ad: 61.64,
            period: 126000.0, omega: 341.71, w: 173.77, h: Some(4.71), class: "TNO".to_string(),
        },

        // ═══════════════════════════════════════════════════════════════
        // CENTAURS AND TRANSITION OBJECTS
        // ═══════════════════════════════════════════════════════════════
        KuiperBeltObject {
            name: "42355 Typhon".to_string(),
            a: 37.71, e: 0.5367, i: 2.43, q: 17.470, ad: 57.94,
            period: 84600.0, omega: 351.86, w: 158.75, h: Some(7.63), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "65489 Ceto".to_string(),
            a: 100.5, e: 0.8238, i: 22.30, q: 17.709, ad: 183.25,
            period: 368000.0, omega: 171.95, w: 319.46, h: Some(6.47), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "309239 (2007 RW10)".to_string(),
            a: 30.34, e: 0.2977, i: 36.08, q: 21.305, ad: 39.37,
            period: 61000.0, omega: 187.03, w: 96.73, h: Some(6.72), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "316179 (2010 EN65)".to_string(),
            a: 30.67, e: 0.3146, i: 19.27, q: 21.020, ad: 40.31,
            period: 62000.0, omega: 234.29, w: 225.19, h: Some(7.16), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "385571 Otrera".to_string(),
            a: 30.32, e: 0.0319, i: 1.43, q: 29.352, ad: 31.29,
            period: 61000.0, omega: 34.80, w: 7.78, h: Some(8.91), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "385695 Clete".to_string(),
            a: 30.31, e: 0.0568, i: 5.25, q: 28.590, ad: 32.03,
            period: 61000.0, omega: 169.38, w: 300.67, h: Some(8.49), class: "TNO".to_string(),
        },

        // ═══════════════════════════════════════════════════════════════
        // ADDITIONAL CLASSICAL AND RESONANT OBJECTS
        // ═══════════════════════════════════════════════════════════════
        KuiperBeltObject {
            name: "15807 (1994 GV9)".to_string(),
            a: 43.66, e: 0.0635, i: 0.57, q: 40.891, ad: 46.43,
            period: 105000.0, omega: 177.00, w: 302.79, h: Some(7.32), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "15809 (1994 JS)".to_string(),
            a: 42.02, e: 0.2135, i: 14.06, q: 33.046, ad: 50.99,
            period: 99500.0, omega: 56.41, w: 237.64, h: Some(7.64), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "15820 (1994 TB)".to_string(),
            a: 39.86, e: 0.3229, i: 12.13, q: 26.986, ad: 52.73,
            period: 91900.0, omega: 317.45, w: 99.38, h: Some(7.60), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "15836 (1995 DA2)".to_string(),
            a: 36.42, e: 0.0766, i: 6.55, q: 33.627, ad: 39.21,
            period: 80300.0, omega: 127.41, w: 334.45, h: Some(8.06), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "15875 (1996 TP66)".to_string(),
            a: 39.74, e: 0.3345, i: 5.68, q: 26.446, ad: 53.03,
            period: 91500.0, omega: 316.82, w: 76.17, h: Some(7.36), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "15883 (1997 CR29)".to_string(),
            a: 47.19, e: 0.2155, i: 19.12, q: 37.019, ad: 57.35,
            period: 118000.0, omega: 127.09, w: 301.57, h: Some(7.07), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "16684 (1994 JQ1)".to_string(),
            a: 43.87, e: 0.0457, i: 3.75, q: 41.867, ad: 45.87,
            period: 106000.0, omega: 25.66, w: 253.04, h: Some(6.71), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "19255 (1994 VK8)".to_string(),
            a: 43.09, e: 0.0341, i: 1.49, q: 41.617, ad: 44.56,
            period: 103000.0, omega: 72.37, w: 104.08, h: Some(6.94), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "19299 (1996 SZ4)".to_string(),
            a: 39.93, e: 0.2636, i: 4.74, q: 29.409, ad: 50.46,
            period: 92200.0, omega: 15.96, w: 31.02, h: Some(8.47), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "19308 (1996 TO66)".to_string(),
            a: 43.46, e: 0.1142, i: 27.43, q: 38.499, ad: 48.42,
            period: 105000.0, omega: 355.24, w: 240.43, h: Some(4.80), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "20108 (1995 QZ9)".to_string(),
            a: 39.7, e: 0.1456, i: 19.55, q: 33.918, ad: 45.47,
            period: 91300.0, omega: 187.99, w: 145.33, h: Some(7.64), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "24835 (1995 SM55)".to_string(),
            a: 42.12, e: 0.1111, i: 27.03, q: 37.438, ad: 46.80,
            period: 99800.0, omega: 21.00, w: 70.47, h: Some(4.61), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "24952 (1997 QJ4)".to_string(),
            a: 39.69, e: 0.2325, i: 16.56, q: 30.461, ad: 48.92,
            period: 91300.0, omega: 346.86, w: 82.06, h: Some(7.50), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "24978 (1998 HJ151)".to_string(),
            a: 43.13, e: 0.0490, i: 2.40, q: 41.012, ad: 45.24,
            period: 103000.0, omega: 50.46, w: 121.58, h: Some(7.28), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "26308 (1998 SM165)".to_string(),
            a: 47.91, e: 0.3699, i: 13.49, q: 30.189, ad: 65.63,
            period: 121000.0, omega: 183.12, w: 131.84, h: Some(5.77), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "29981 (1999 TD10)".to_string(),
            a: 98.47, e: 0.8743, i: 5.96, q: 12.374, ad: 184.56,
            period: 357000.0, omega: 184.61, w: 173.03, h: Some(8.80), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "32929 (1995 QY9)".to_string(),
            a: 39.93, e: 0.2666, i: 4.83, q: 29.285, ad: 50.58,
            period: 92200.0, omega: 342.12, w: 26.18, h: Some(8.66), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "33001 (1997 CU29)".to_string(),
            a: 43.63, e: 0.0346, i: 1.45, q: 42.117, ad: 45.14,
            period: 105000.0, omega: 350.27, w: 263.15, h: Some(6.42), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "33128 (1998 BU48)".to_string(),
            a: 33.32, e: 0.3851, i: 14.23, q: 20.491, ad: 46.16,
            period: 70300.0, omega: 132.67, w: 282.71, h: Some(6.93), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "33340 (1998 VG44)".to_string(),
            a: 39.58, e: 0.2579, i: 3.03, q: 29.372, ad: 49.79,
            period: 91000.0, omega: 127.96, w: 324.62, h: Some(6.42), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "35671 (1998 SN165)".to_string(),
            a: 38.08, e: 0.0469, i: 4.60, q: 36.297, ad: 39.87,
            period: 85800.0, omega: 192.08, w: 260.86, h: Some(5.60), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "38083 Rhadamanthus".to_string(),
            a: 38.91, e: 0.1569, i: 12.76, q: 32.806, ad: 45.01,
            period: 88700.0, omega: 10.00, w: 79.83, h: Some(7.03), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "38084 (1999 HB12)".to_string(),
            a: 55.26, e: 0.4110, i: 13.15, q: 32.543, ad: 77.97,
            period: 150000.0, omega: 166.42, w: 66.33, h: Some(7.06), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "40314 (1999 KR16)".to_string(),
            a: 48.48, e: 0.2999, i: 24.83, q: 33.940, ad: 63.02,
            period: 123000.0, omega: 205.67, w: 58.81, h: Some(5.59), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "42301 (2001 UR163)".to_string(),
            a: 51.77, e: 0.2803, i: 0.75, q: 37.256, ad: 66.28,
            period: 136000.0, omega: 302.40, w: 343.47, h: Some(4.14), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "44594 (1999 OX3)".to_string(),
            a: 32.66, e: 0.4609, i: 2.62, q: 17.604, ad: 47.71,
            period: 68200.0, omega: 259.17, w: 144.56, h: Some(7.08), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "45802 (2000 PV29)".to_string(),
            a: 43.4, e: 0.0101, i: 1.18, q: 42.962, ad: 43.84,
            period: 104000.0, omega: 173.19, w: 51.53, h: Some(7.87), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "47932 (2000 GN171)".to_string(),
            a: 39.15, e: 0.2783, i: 10.82, q: 28.252, ad: 50.05,
            period: 89500.0, omega: 26.13, w: 194.59, h: Some(6.28), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "48639 (1995 TL8)".to_string(),
            a: 52.91, e: 0.2387, i: 0.24, q: 40.285, ad: 65.54,
            period: 141000.0, omega: 260.79, w: 85.66, h: Some(4.85), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "49673 (1999 RA215)".to_string(),
            a: 43.2, e: 0.1062, i: 22.55, q: 38.609, ad: 47.78,
            period: 104000.0, omega: 132.38, w: 270.22, h: Some(7.61), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "52747 (1998 HM151)".to_string(),
            a: 44.19, e: 0.0563, i: 0.54, q: 41.703, ad: 46.68,
            period: 107000.0, omega: 63.86, w: 251.70, h: Some(7.78), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "54520 (2000 PJ30)".to_string(),
            a: 121.9, e: 0.7654, i: 5.72, q: 28.603, ad: 215.22,
            period: 492000.0, omega: 293.41, w: 303.43, h: Some(7.99), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "55636 (2002 TX300)".to_string(),
            a: 43.48, e: 0.1213, i: 25.86, q: 38.204, ad: 48.75,
            period: 105000.0, omega: 324.67, w: 342.67, h: Some(3.50), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "55637 Uni".to_string(),
            a: 42.97, e: 0.1464, i: 19.41, q: 36.683, ad: 49.27,
            period: 103000.0, omega: 204.59, w: 275.64, h: Some(3.82), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "55638 (2002 VE95)".to_string(),
            a: 39.64, e: 0.2926, i: 16.32, q: 28.042, ad: 51.24,
            period: 91200.0, omega: 199.75, w: 208.00, h: Some(5.59), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "59358 (1999 CL158)".to_string(),
            a: 41.58, e: 0.2108, i: 10.02, q: 32.819, ad: 50.35,
            period: 97900.0, omega: 120.00, w: 328.68, h: Some(6.91), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "60454 (2000 CH105)".to_string(),
            a: 44.28, e: 0.0838, i: 1.16, q: 40.571, ad: 47.99,
            period: 108000.0, omega: 319.93, w: 291.20, h: Some(6.74), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "60458 (2000 CM114)".to_string(),
            a: 59.61, e: 0.4045, i: 19.68, q: 35.497, ad: 83.72,
            period: 168000.0, omega: 312.27, w: 251.15, h: Some(7.38), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "60608 (2000 EE173)".to_string(),
            a: 49.04, e: 0.5405, i: 5.96, q: 22.534, ad: 75.54,
            period: 125000.0, omega: 293.95, w: 235.24, h: Some(8.47), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "60620 (2000 FD8)".to_string(),
            a: 43.57, e: 0.2177, i: 19.52, q: 34.081, ad: 53.06,
            period: 105000.0, omega: 184.84, w: 81.39, h: Some(6.63), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "60621 (2000 FE8)".to_string(),
            a: 55.2, e: 0.4033, i: 5.87, q: 32.936, ad: 77.46,
            period: 150000.0, omega: 3.90, w: 142.75, h: Some(6.86), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "66452 (1999 OF4)".to_string(),
            a: 44.95, e: 0.0624, i: 2.66, q: 42.143, ad: 47.75,
            period: 110000.0, omega: 134.42, w: 86.96, h: Some(6.81), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "69986 (1998 WW24)".to_string(),
            a: 39.73, e: 0.2273, i: 13.91, q: 30.704, ad: 48.76,
            period: 91500.0, omega: 233.88, w: 147.98, h: Some(8.58), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "69987 (1998 WA25)".to_string(),
            a: 42.82, e: 0.0223, i: 1.05, q: 41.868, ad: 43.78,
            period: 102000.0, omega: 136.31, w: 233.14, h: Some(7.08), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "69988 (1998 WA31)".to_string(),
            a: 55.58, e: 0.4286, i: 9.45, q: 31.758, ad: 79.41,
            period: 151000.0, omega: 20.69, w: 310.56, h: Some(6.86), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "69990 (1998 WU31)".to_string(),
            a: 39.53, e: 0.1886, i: 6.57, q: 32.071, ad: 46.98,
            period: 90800.0, omega: 237.06, w: 143.70, h: Some(8.18), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "73480 (2002 PN34)".to_string(),
            a: 31.03, e: 0.5674, i: 16.61, q: 13.425, ad: 48.64,
            period: 63100.0, omega: 299.27, w: 358.71, h: Some(8.59), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "76803 (2000 PK30)".to_string(),
            a: 38.62, e: 0.1205, i: 33.77, q: 33.967, ad: 43.27,
            period: 87700.0, omega: 127.45, w: 127.08, h: Some(7.29), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "78799 Xewioso".to_string(),
            a: 37.69, e: 0.2435, i: 14.34, q: 28.511, ad: 46.86,
            period: 84500.0, omega: 46.73, w: 248.38, h: Some(4.86), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "79969 (1999 CP133)".to_string(),
            a: 34.85, e: 0.0848, i: 3.18, q: 31.893, ad: 37.80,
            period: 75100.0, omega: 334.15, w: 156.18, h: Some(7.74), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "79978 (1999 CC158)".to_string(),
            a: 54.16, e: 0.2791, i: 18.72, q: 39.042, ad: 69.28,
            period: 146000.0, omega: 336.99, w: 101.87, h: Some(5.74), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "79983 (1999 DF9)".to_string(),
            a: 46.4, e: 0.1442, i: 9.81, q: 39.713, ad: 53.10,
            period: 115000.0, omega: 334.80, w: 176.22, h: Some(6.08), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "80806 (2000 CM105)".to_string(),
            a: 42.27, e: 0.0679, i: 3.76, q: 39.404, ad: 45.14,
            period: 100000.0, omega: 45.58, w: 8.03, h: Some(6.69), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "82155 (2001 FZ173)".to_string(),
            a: 84.62, e: 0.6176, i: 12.72, q: 32.362, ad: 136.88,
            period: 284000.0, omega: 2.40, w: 199.01, h: Some(6.16), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "82157 (2001 FM185)".to_string(),
            a: 38.53, e: 0.0568, i: 5.37, q: 36.343, ad: 40.72,
            period: 87400.0, omega: 150.73, w: 114.35, h: Some(7.06), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "420356 Praamzius".to_string(),
            a: 42.89, e: 0.0044, i: 1.09, q: 42.699, ad: 43.08,
            period: 103000.0, omega: 314.16, w: 48.43, h: Some(5.75), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "202421 (2005 UQ513)".to_string(),
            a: 43.53, e: 0.1452, i: 25.72, q: 37.207, ad: 49.85,
            period: 105000.0, omega: 307.89, w: 219.57, h: Some(3.91), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "120178 (2003 OP32)".to_string(),
            a: 43.18, e: 0.1034, i: 27.15, q: 38.710, ad: 47.64,
            period: 104000.0, omega: 183.01, w: 68.76, h: Some(4.00), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "119951 (2002 KX14)".to_string(),
            a: 38.62, e: 0.0448, i: 0.41, q: 36.893, ad: 40.35,
            period: 87700.0, omega: 286.67, w: 79.04, h: Some(4.71), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "90568 Goibniu".to_string(),
            a: 41.81, e: 0.0760, i: 22.04, q: 38.631, ad: 44.99,
            period: 98700.0, omega: 250.58, w: 289.67, h: Some(3.96), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "175113 (2004 PF115)".to_string(),
            a: 38.98, e: 0.0672, i: 13.36, q: 36.359, ad: 41.60,
            period: 88900.0, omega: 84.74, w: 82.08, h: Some(4.47), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "144897 (2004 UX10)".to_string(),
            a: 39.23, e: 0.0384, i: 9.53, q: 37.718, ad: 40.73,
            period: 89700.0, omega: 148.02, w: 160.00, h: Some(4.38), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "120348 (2004 TY364)".to_string(),
            a: 39.09, e: 0.0676, i: 24.83, q: 36.449, ad: 41.74,
            period: 89300.0, omega: 140.58, w: 353.43, h: Some(4.31), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "145453 (2005 RR43)".to_string(),
            a: 43.52, e: 0.1409, i: 28.44, q: 37.388, ad: 49.65,
            period: 105000.0, omega: 85.87, w: 281.79, h: Some(4.13), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "230965 (2004 XA192)".to_string(),
            a: 47.5, e: 0.2533, i: 38.08, q: 35.472, ad: 59.53,
            period: 120000.0, omega: 328.69, w: 132.22, h: Some(4.17), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "308193 (2005 CB79)".to_string(),
            a: 43.45, e: 0.1428, i: 28.60, q: 37.243, ad: 49.65,
            period: 105000.0, omega: 112.72, w: 90.48, h: Some(4.66), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "444030 (2004 NT33)".to_string(),
            a: 43.42, e: 0.1485, i: 31.21, q: 36.966, ad: 49.86,
            period: 104000.0, omega: 241.15, w: 38.18, h: Some(4.42), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "278361 (2007 JJ43)".to_string(),
            a: 47.71, e: 0.1549, i: 12.09, q: 40.321, ad: 55.10,
            period: 120000.0, omega: 272.50, w: 8.38, h: Some(4.49), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "312645 (2010 EP65)".to_string(),
            a: 47.43, e: 0.3038, i: 18.91, q: 33.022, ad: 61.85,
            period: 119000.0, omega: 205.04, w: 351.65, h: Some(5.45), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "315530 (2008 AP129)".to_string(),
            a: 41.99, e: 0.1427, i: 27.41, q: 36.003, ad: 47.99,
            period: 99400.0, omega: 14.87, w: 59.21, h: Some(4.82), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "416400 (2003 UZ117)".to_string(),
            a: 44.59, e: 0.1396, i: 27.40, q: 38.364, ad: 50.82,
            period: 109000.0, omega: 204.58, w: 246.26, h: Some(5.23), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "386723 (2009 YE7)".to_string(),
            a: 44.59, e: 0.1373, i: 29.07, q: 38.472, ad: 50.71,
            period: 109000.0, omega: 141.66, w: 99.31, h: Some(4.57), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "305543 (2008 QY40)".to_string(),
            a: 62.37, e: 0.4097, i: 25.12, q: 36.816, ad: 87.91,
            period: 180000.0, omega: 43.25, w: 332.26, h: Some(5.38), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "451657 (2012 WD36)".to_string(),
            a: 77.71, e: 0.5172, i: 23.68, q: 37.517, ad: 117.90,
            period: 250000.0, omega: 177.32, w: 293.19, h: Some(6.84), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "353222 (2009 YD7)".to_string(),
            a: 125.7, e: 0.8936, i: 30.77, q: 13.379, ad: 238.05,
            period: 515000.0, omega: 125.99, w: 326.91, h: Some(10.08), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "341520 Mors-Somnus".to_string(),
            a: 39.56, e: 0.2702, i: 11.27, q: 28.871, ad: 50.25,
            period: 90900.0, omega: 196.67, w: 206.05, h: Some(6.79), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "148780 Altjira".to_string(),
            a: 44.54, e: 0.0568, i: 5.20, q: 42.009, ad: 47.07,
            period: 109000.0, omega: 1.89, w: 303.85, h: Some(5.77), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "385446 Manwe".to_string(),
            a: 43.84, e: 0.1164, i: 2.67, q: 38.733, ad: 48.94,
            period: 106000.0, omega: 68.57, w: 19.24, h: Some(6.57), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "181708 (1993 FW)".to_string(),
            a: 43.56, e: 0.0449, i: 7.75, q: 41.610, ad: 45.52,
            period: 105000.0, omega: 187.94, w: 41.18, h: Some(6.88), class: "TNO".to_string(),
        },
        KuiperBeltObject {
            name: "126154 (2001 YH140)".to_string(),
            a: 42.54, e: 0.1450, i: 11.06, q: 36.375, ad: 48.71,
            period: 101000.0, omega: 108.77, w: 355.66, h: Some(5.54), class: "TNO".to_string(),
        },
    ]
}

/// Returns a subset of well-known TNOs for quick testing
pub fn get_sample_kbos() -> Vec<KuiperBeltObject> {
    get_kbo_data().into_iter().take(30).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_data_loaded() {
        let objects = get_kbo_data();
        assert!(!objects.is_empty());
        assert!(objects.len() > 100); // Should have substantial data
    }

    #[test]
    fn test_known_objects() {
        let objects = get_kbo_data();

        // Verify Pluto exists
        let pluto = objects.iter().find(|o| o.name.contains("Pluto"));
        assert!(pluto.is_some());

        // Verify Eris exists
        let eris = objects.iter().find(|o| o.name.contains("Eris"));
        assert!(eris.is_some());

        // Verify Sedna exists
        let sedna = objects.iter().find(|o| o.name.contains("Sedna"));
        assert!(sedna.is_some());
    }

    #[test]
    fn test_orbital_parameters_valid() {
        let objects = get_kbo_data();

        for obj in &objects {
            // Semi-major axis should be positive
            assert!(obj.a > 0.0, "Invalid a for {}", obj.name);

            // Eccentricity should be 0-1 (or slightly > 1 for hyperbolic)
            assert!(obj.e >= 0.0 && obj.e < 1.5, "Invalid e for {}", obj.name);

            // Inclination should be 0-180
            assert!(obj.i >= 0.0 && obj.i <= 180.0, "Invalid i for {}", obj.name);
        }
    }

    #[test]
    fn test_sample_data() {
        let sample = get_sample_kbos();
        assert_eq!(sample.len(), 30);
    }
}
