/**\
 * Copyright (c) 2022 Bosch Sensortec GmbH. All rights reserved.
 *
 * SPDX-License-Identifier: BSD-3-Clause
 **/

#include <stdint.h>
#include <stdlib.h>
#include <stdio.h>
#include <time.h>

#include "bmp3.h"
#include "bmp3_defs.h"
#include "common.h"
#include "bmp388_i2c.h"

static uint8_t g_dev_addr;

/*
 * Debug msgs to stdout
 */
// #define DEBUG

/*
 * I2C read function map to COINES platform
 */
BMP3_INTF_RET_TYPE
bmp3_i2c_read(uint8_t reg_addr, uint8_t *reg_data, uint32_t len, void *intf_ptr)
{
    uint8_t device_addr = *(uint8_t*)intf_ptr;

    (void)intf_ptr;

    return bmp388_read_register(reg_addr, reg_data, (size_t)len);
}

/*
 * I2C write function map to COINES platform
 */
BMP3_INTF_RET_TYPE
bmp3_i2c_write(uint8_t reg_addr, const uint8_t *reg_data, uint32_t len, void *intf_ptr)
{
    uint8_t device_addr = *(uint8_t*)intf_ptr;

    (void)intf_ptr;

    return bmp388_write_register(reg_addr, *reg_data);
}

/*
 * SPI read function map to COINES platform
 */
BMP3_INTF_RET_TYPE
bmp3_spi_read(uint8_t reg_addr, uint8_t *reg_data, uint32_t len, void *intf_ptr)
{
    uint8_t device_addr = *(uint8_t*)intf_ptr;

    (void)intf_ptr;

    //return coines_read_spi(COINES_SPI_BUS_0, device_addr, reg_addr, reg_data, (uint16_t)len);
}

/*
 * SPI write function map to COINES platform
 */
BMP3_INTF_RET_TYPE
bmp3_spi_write(uint8_t reg_addr, const uint8_t *reg_data, uint32_t len, void *intf_ptr)
{
    uint8_t device_addr = *(uint8_t*)intf_ptr;

    (void)intf_ptr;

    //return coines_write_spi(COINES_SPI_BUS_0, device_addr, reg_addr, (uint8_t *)reg_data, (uint16_t)len);
}

/*
 * Delay function
 */
void
bmp3_delay_us(uint32_t period, void *intf_ptr)
{
    (void)intf_ptr;
    time_t usec = period * 1000;
    struct timespec ts = {.tv_sec = 0, .tv_nsec = usec};

    nanosleep(&ts, &ts);
}

void
bmp3_check_rslt(const char api_name[], int8_t rslt)
{
    switch (rslt)
    {
        case BMP3_OK:

            /* Do nothing */
            break;
        case BMP3_E_NULL_PTR:
            printf("API [%s] Error [%d] : Null pointer\r\n", api_name, rslt);
            break;
        case BMP3_E_COMM_FAIL:
            printf("API [%s] Error [%d] : Communication failure\r\n", api_name, rslt);
            break;
        case BMP3_E_INVALID_LEN:
            printf("API [%s] Error [%d] : Incorrect length parameter\r\n", api_name, rslt);
            break;
        case BMP3_E_DEV_NOT_FOUND:
            printf("API [%s] Error [%d] : Device not found\r\n", api_name, rslt);
            break;
        case BMP3_E_CONFIGURATION_ERR:
            printf("API [%s] Error [%d] : Configuration Error\r\n", api_name, rslt);
            break;
        case BMP3_W_SENSOR_NOT_ENABLED:
            printf("API [%s] Error [%d] : Warning when Sensor not enabled\r\n", api_name, rslt);
            break;
        case BMP3_W_INVALID_FIFO_REQ_FRAME_CNT:
            printf("API [%s] Error [%d] : Warning when Fifo watermark level is not in limit\r\n", api_name, rslt);
            break;
        default:
            printf("API [%s] Error [%d] : Unknown error code\r\n", api_name, rslt);
            break;
    }
}

BMP3_INTF_RET_TYPE
bmp3_interface_init(struct bmp3_dev *bmp3, uint8_t intf)
{
    int8_t rslt = BMP3_OK;

#ifdef DEBUG
    printf("I2C Interface\n");
#endif
    g_dev_addr = BMP3_ADDR_I2C_PRIM;
    bmp3->read = bmp3_i2c_read;
    bmp3->write = bmp3_i2c_write;
    bmp3->intf = intf;
    bmp3->delay_us = bmp3_delay_us;
    bmp3->intf_ptr = &g_dev_addr;

    if (bmp388_init() != 0) {
        fprintf(stderr, "bmp388_init failure");
        exit(1);
    }

    bmp3_delay_us(1000 * 1000, NULL);

    return rslt;
}

void
bmp3_deinit(void)
{
    (void)fflush(stdout);

    bmp3_delay_us(1000 * 1000, NULL);

    /* sensor reset */
    bmp388_reset();
    bmp3_delay_us(1000 * 1000, NULL);
    //bmp388_deinit();
}
