#ifndef __BMP388_I2C_H__
#define __BMP388_I2C_H__

int bmp388_write_register(uint8_t, uint8_t);
int bmp388_read_register(uint8_t, uint8_t *, size_t);
int bmp388_init();
int bmp388_reset();
int bmp388_deinit();
#endif // __BMP388_I2C_H__
