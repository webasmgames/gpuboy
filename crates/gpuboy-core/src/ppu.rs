pub struct Ppu {
    pub lcdc: u8,
    pub stat: u8,
    pub scy: u8,
    pub scx: u8,
    pub ly: u8,
    pub lyc: u8,
    pub bgp: u8,
    pub obp0: u8,
    pub obp1: u8,
    pub wy: u8,
    pub wx: u8,

    dot: u32,
    mode: u8,
    window_line: u8,
    framebuffer: [u8; 160 * 144 * 4],
    back_buffer: [u8; 160 * 144 * 4],
}

impl Default for Ppu {
    fn default() -> Self {
        Self::new()
    }
}

impl Ppu {
    pub fn new() -> Self {
        Ppu {
            lcdc: 0x91,
            stat: 0x82,
            scy: 0,
            scx: 0,
            ly: 0,
            lyc: 0,
            bgp: 0xFC,
            obp0: 0,
            obp1: 0,
            wy: 0,
            wx: 0,
            dot: 0,
            mode: 2,
            window_line: 0,
            framebuffer: [0xFF; 160 * 144 * 4],
            back_buffer: [0xFF; 160 * 144 * 4],
        }
    }

    pub fn get_framebuffer(&self) -> &[u8] {
        &self.back_buffer
    }

    pub fn step(&mut self, t_cycles: u32, vram: &[u8], oam: &[u8], interrupt_flags: &mut u8) {
        if self.lcdc & 0x80 == 0 {
            self.framebuffer = [0xFF; 160 * 144 * 4];
            self.ly = 0;
            self.dot = 0;
            self.mode = 0;
            self.stat &= 0xF8;
            return;
        }

        self.dot += t_cycles;

        while self.dot >= 456 {
            self.dot -= 456;
            self.advance_ly(interrupt_flags);
        }

        self.update_mode(vram, oam, interrupt_flags);
    }

    fn advance_ly(&mut self, interrupt_flags: &mut u8) {
        self.ly += 1;

        if self.ly == 144 {
            self.mode = 1;
            self.stat = (self.stat & 0xF8) | 0x01;
            self.back_buffer = self.framebuffer;
            *interrupt_flags |= 0x01;
            if self.stat & 0x10 != 0 {
                *interrupt_flags |= 0x02;
            }
        } else if self.ly > 153 {
            self.ly = 0;
            self.window_line = 0;
            self.mode = 2;
            self.stat = (self.stat & 0xF8) | 0x02;
            if self.stat & 0x20 != 0 {
                *interrupt_flags |= 0x02;
            }
        } else if self.ly < 144 {
            self.mode = 2;
            self.stat = (self.stat & 0xF8) | 0x02;
            if self.stat & 0x20 != 0 {
                *interrupt_flags |= 0x02;
            }
        }

        self.check_lyc(interrupt_flags);
    }

    fn update_mode(&mut self, vram: &[u8], oam: &[u8], interrupt_flags: &mut u8) {
        if self.ly >= 144 {
            return;
        }

        let new_mode = if self.dot < 80 {
            2
        } else if self.dot < 252 {
            3
        } else {
            0
        };

        if new_mode != self.mode {
            self.mode = new_mode;
            match self.mode {
                3 => {
                    self.render_scanline(vram, oam);
                }
                0 if self.stat & 0x08 != 0 => {
                    *interrupt_flags |= 0x02;
                }
                2 if self.stat & 0x20 != 0 => {
                    *interrupt_flags |= 0x02;
                }
                _ => {}
            }
        }

        self.stat = (self.stat & 0xF8) | (self.mode & 0x03);
    }

    fn check_lyc(&mut self, interrupt_flags: &mut u8) {
        if self.ly == self.lyc {
            self.stat |= 0x04;
            if self.stat & 0x40 != 0 {
                *interrupt_flags |= 0x02;
            }
        } else {
            self.stat &= !0x04;
        }
    }

    fn render_scanline(&mut self, vram: &[u8], oam: &[u8]) {
        let ly = self.ly as usize;
        let mut bg_indices = [0u8; 160];

        // Background layer
        if self.lcdc & 0x01 == 0 {
            let color = palette_color(self.bgp, 0);
            for x in 0..160usize {
                let base = (ly * 160 + x) * 4;
                self.framebuffer[base..base + 4].copy_from_slice(&color);
            }
            // bg_indices already zero-initialized
        } else {
            let map_base: usize = if self.lcdc & 0x08 != 0 {
                0x9C00
            } else {
                0x9800
            };
            for (x, bg_idx) in bg_indices.iter_mut().enumerate() {
                let bg_x = (self.scx as usize + x) % 256;
                let bg_y = (self.scy as usize + ly) % 256;
                let tile_idx = vram[map_base - 0x8000 + (bg_y / 8) * 32 + (bg_x / 8)];
                let tile_base = if self.lcdc & 0x10 != 0 {
                    (tile_idx as usize) * 16
                } else {
                    let signed = tile_idx as i8 as isize;
                    (0x9000usize - 0x8000).wrapping_add((signed * 16) as usize)
                };
                let row = bg_y % 8;
                let col = bg_x % 8;
                let idx = tile_pixel(vram, tile_base, row, col);
                *bg_idx = idx;
                let color = palette_color(self.bgp, idx);
                let base = (ly * 160 + x) * 4;
                self.framebuffer[base..base + 4].copy_from_slice(&color);
            }
        }

        // Window layer
        if self.lcdc & 0x20 != 0 && self.wy <= self.ly && self.wx <= 166 {
            let win_map_base: usize = if self.lcdc & 0x40 != 0 {
                0x9C00
            } else {
                0x9800
            };
            let wx_start = (self.wx as usize).saturating_sub(7);
            let win_y = self.window_line as usize;
            let mut drew_window = false;
            for (x, bg_idx) in bg_indices.iter_mut().enumerate().skip(wx_start) {
                let win_x = x - wx_start;
                let tile_idx = vram[win_map_base - 0x8000 + (win_y / 8) * 32 + (win_x / 8)];
                let tile_base = if self.lcdc & 0x10 != 0 {
                    (tile_idx as usize) * 16
                } else {
                    let signed = tile_idx as i8 as isize;
                    (0x9000usize - 0x8000).wrapping_add((signed * 16) as usize)
                };
                let row = win_y % 8;
                let col = win_x % 8;
                let idx = tile_pixel(vram, tile_base, row, col);
                *bg_idx = idx;
                let color = palette_color(self.bgp, idx);
                let base = (ly * 160 + x) * 4;
                self.framebuffer[base..base + 4].copy_from_slice(&color);
                drew_window = true;
            }
            if drew_window {
                self.window_line += 1;
            }
        }

        // Sprite layer
        if self.lcdc & 0x02 != 0 {
            #[derive(Copy, Clone)]
            struct Sprite {
                x: u8,
                y: u8,
                tile: u8,
                attrs: u8,
            }

            let zero = Sprite {
                x: 0,
                y: 0,
                tile: 0,
                attrs: 0,
            };
            let mut sprites = [zero; 10];
            let mut count = 0usize;

            for i in 0..40usize {
                let base = i * 4;
                let sy = oam[base];
                let sx = oam[base + 1];
                let tile = oam[base + 2];
                let attrs = oam[base + 3];
                let ly16 = ly as u16 + 16;
                if ly16 >= sy as u16 && ly16 < sy as u16 + 8 {
                    sprites[count] = Sprite {
                        x: sx,
                        y: sy,
                        tile,
                        attrs,
                    };
                    count += 1;
                    if count == 10 {
                        break;
                    }
                }
            }

            // Sort by X ascending (stable sort for priority)
            for i in 1..count {
                let mut j = i;
                while j > 0 && sprites[j].x < sprites[j - 1].x {
                    sprites.swap(j, j - 1);
                    j -= 1;
                }
            }

            for sp in sprites[..count].iter() {
                let palette = if sp.attrs & 0x10 != 0 {
                    self.obp1
                } else {
                    self.obp0
                };
                let tile_row_raw = ly + 16 - sp.y as usize;
                for col in 0..8usize {
                    let screen_x = sp.x as isize - 8 + col as isize;
                    if !(0_isize..160).contains(&screen_x) {
                        continue;
                    }
                    let sx = screen_x as usize;

                    let row = if sp.attrs & 0x40 != 0 {
                        7 - tile_row_raw
                    } else {
                        tile_row_raw
                    };
                    let c = if sp.attrs & 0x20 != 0 { 7 - col } else { col };

                    let tile_base = (sp.tile as usize) * 16;
                    let idx = tile_pixel(vram, tile_base, row, c);
                    if idx == 0 {
                        continue;
                    }

                    if sp.attrs & 0x80 != 0 && bg_indices[sx] != 0 {
                        continue;
                    }

                    let color = palette_color(palette, idx);
                    let base = (ly * 160 + sx) * 4;
                    self.framebuffer[base..base + 4].copy_from_slice(&color);
                }
            }
        }
    }
}

fn palette_color(palette: u8, idx: u8) -> [u8; 4] {
    let shade = (palette >> (idx * 2)) & 0x03;
    match shade {
        0 => [0xE0, 0xF8, 0xD0, 0xFF],
        1 => [0x88, 0xC0, 0x70, 0xFF],
        2 => [0x34, 0x68, 0x56, 0xFF],
        _ => [0x08, 0x18, 0x20, 0xFF],
    }
}

fn tile_pixel(vram: &[u8], tile_base: usize, row: usize, col: usize) -> u8 {
    let lo = vram[tile_base + row * 2];
    let hi = vram[tile_base + row * 2 + 1];
    let bit = 7 - col;
    ((lo >> bit) & 1) | (((hi >> bit) & 1) << 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_vram() -> [u8; 0x2000] {
        [0u8; 0x2000]
    }
    fn empty_oam() -> [u8; 0xA0] {
        [0u8; 0xA0]
    }

    #[test]
    fn test_mode_sequence() {
        let mut ppu = Ppu::new();
        ppu.ly = 0;
        ppu.dot = 0;
        ppu.mode = 2;
        let vram = empty_vram();
        let oam = empty_oam();
        let mut if_reg = 0u8;

        // After 80 T-cycles: still in Mode 2 (dot=80 triggers transition to 3)
        ppu.step(79, &vram, &oam, &mut if_reg);
        assert_eq!(ppu.mode, 2);

        // One more cycle: dot=80, transition to Mode 3
        ppu.step(1, &vram, &oam, &mut if_reg);
        assert_eq!(ppu.mode, 3);

        // After 172 more T-cycles (dot=252): transition to Mode 0
        ppu.step(172, &vram, &oam, &mut if_reg);
        assert_eq!(ppu.mode, 0);

        // After 204 more T-cycles: dot=456, LY increments to 1, mode=2
        ppu.step(204, &vram, &oam, &mut if_reg);
        assert_eq!(ppu.ly, 1);
        assert_eq!(ppu.mode, 2);
    }

    #[test]
    fn test_vblank_interrupt() {
        let mut ppu = Ppu::new();
        ppu.ly = 0;
        ppu.dot = 0;
        ppu.mode = 2;
        let vram = empty_vram();
        let oam = empty_oam();
        let mut if_reg = 0u8;

        // 144 scanlines × 456 T-cycles each
        ppu.step(144 * 456, &vram, &oam, &mut if_reg);
        assert_eq!(ppu.ly, 144);
        assert!(if_reg & 0x01 != 0, "VBlank interrupt not set");
    }

    #[test]
    fn test_lyc_stat_interrupt() {
        let mut ppu = Ppu::new();
        ppu.ly = 0;
        ppu.dot = 0;
        ppu.mode = 2;
        ppu.lyc = 5;
        ppu.stat |= 0x40;
        let vram = empty_vram();
        let oam = empty_oam();
        let mut if_reg = 0u8;

        // Step to LY=5
        ppu.step(5 * 456, &vram, &oam, &mut if_reg);
        assert_eq!(ppu.ly, 5);
        assert!(if_reg & 0x02 != 0, "STAT LYC=LY interrupt not set");
    }

    #[test]
    fn test_bg_rendering() {
        let mut vram = empty_vram();
        // Tile 0 at 0x8000: row 0 = lo=0xAA, hi=0x00 → alternating indices 0,1,0,1,0,1,0,1
        // pixel 0: bit 7 of lo=1 (index bit0=1), bit 7 of hi=0 (index bit1=0) → color index 1
        vram[0] = 0xAA; // lo byte of row 0 of tile 0
        vram[1] = 0x00; // hi byte
                        // Tile map at 0x9800 (offset 0x1800 in vram): tile index 0 at map[0]
        vram[0x1800] = 0;

        let mut ppu = Ppu::new();
        ppu.lcdc = 0x91; // LCD on, BG on, tile data at 0x8000
        ppu.bgp = 0b11100100; // identity mapping: idx 0→shade0, idx 1→shade1
        ppu.scx = 0;
        ppu.scy = 0;
        ppu.ly = 0;

        let oam = empty_oam();
        ppu.render_scanline(&vram, &oam);

        // Pixel (0,0): color index 1 → palette_color(0b11100100, 1) = shade 1 = [0x88, 0xC0, 0x70, 0xFF]
        let expected = palette_color(0b11100100, 1);
        assert_eq!(&ppu.framebuffer[0..4], &expected);
    }

    #[test]
    fn test_sprite_rendering() {
        let mut vram = empty_vram();
        // Tile 0 at 0x8000: row 0: lo=0xFF, hi=0x00 → all pixels color index 1
        vram[0] = 0xFF;
        vram[1] = 0x00;

        let mut oam = empty_oam();
        // Sprite 0: Y=16 (screen row 0), X=8 (screen col 0), tile=0, attrs=0
        oam[0] = 16; // Y
        oam[1] = 8; // X
        oam[2] = 0; // tile
        oam[3] = 0; // attrs

        let mut ppu = Ppu::new();
        ppu.lcdc = 0x93; // LCD on, sprites on, BG on
        ppu.bgp = 0xFC;
        ppu.obp0 = 0b11100100; // identity
        ppu.ly = 0;

        ppu.render_scanline(&vram, &oam);

        // Sprite pixel (0,0): color index 1 via OBP0 → shade 1 = [0x88, 0xC0, 0x70, 0xFF]
        let expected = palette_color(0b11100100, 1);
        assert_eq!(&ppu.framebuffer[0..4], &expected);
    }
}
