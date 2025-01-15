/*
 * This is the API for accessing a Bosch BMP388 sensor.
 * Initially, it's called from Rustcode implementing a web server.
 */

#include <stdio.h>

#include "bmp3.h"
#include "common.h"
#include "bmp388_i2c.h"
#include "sensorApi.h"

// Enable to get sensor data to stdout
//#define DEBUG

int
getSensorData(sensor_data_t *sd)
{
    int rslt;
    uint8_t loop = 0;
    uint16_t settings_sel;
    double press, temp;
    struct bmp3_dev dev;
    struct bmp3_data data = { 0 };
    struct bmp3_settings settings = { 0 };
    struct bmp3_status status = { { 0 } };


    if (!sd) return SENSOR_API_NULL_PARAM;

    /* Interface reference is given as a parameter
     *         For I2C : BMP3_I2C_INTF
     *         For SPI : BMP3_SPI_INTF
     */
    rslt = bmp3_interface_init(&dev, BMP3_I2C_INTF);
    bmp3_check_rslt("bmp3_interface_init", rslt);

    rslt = bmp3_init(&dev);
    bmp3_check_rslt("bmp3_init", rslt);

    settings.int_settings.drdy_en = BMP3_ENABLE;
    settings.press_en = BMP3_ENABLE;
    settings.temp_en = BMP3_ENABLE;

    settings_sel = BMP3_SEL_PRESS_EN | BMP3_SEL_TEMP_EN | BMP3_SEL_PRESS_OS | BMP3_SEL_TEMP_OS | BMP3_SEL_ODR | BMP3_SEL_DRDY_EN;

    rslt = bmp3_set_sensor_settings(settings_sel, &settings, &dev);
    bmp3_check_rslt("bmp3_set_sensor_settings", rslt);

    // Continuous measurement
    settings.op_mode = BMP3_MODE_NORMAL;

    // One time measurement, requires a reset and init in order to start again 
    //settings.op_mode = BMP3_MODE_FORCED;

    rslt = bmp3_set_op_mode(&settings, &dev);
    bmp3_check_rslt("bmp3_set_op_mode", rslt);

    press = 0;
    temp = 0;
    while (loop < ITERATION)
    {
        rslt = bmp3_get_status(&status, &dev);
        bmp3_check_rslt("bmp3_get_status", rslt);

        /* Read temperature and pressure data iteratively based on data ready interrupt */
        if ((rslt == BMP3_OK) && (status.intr.drdy == BMP3_ENABLE))
        {
            /*
             * First parameter indicates the type of data to be read
             * BMP3_PRESS_TEMP : To read pressure and temperature data
             * BMP3_TEMP       : To read only temperature data
             * BMP3_PRESS      : To read only pressure data
             */
            rslt = bmp3_get_sensor_data(BMP3_PRESS_TEMP, &data, &dev);
            bmp3_check_rslt("bmp3_get_sensor_data", rslt);

            /* NOTE : Read status register again to clear data ready interrupt status */
            rslt = bmp3_get_status(&status, &dev);
            bmp3_check_rslt("bmp3_get_status", rslt);
#ifdef DEBUG
            printf("Data[%d]  T: %.2f deg C, P: %.2f Pa\n", loop, (data.temperature), (data.pressure));
#endif
            press += data.pressure;
            temp += data.temperature;

            loop = loop + 1;
            // Used if we're in forced mode
            //break;
        }
    }

    // use average values from iteration readings
    sd->temperature = temp / ITERATION;
    sd->pressure = press / ITERATION;
        
    bmp388_deinit();

    return SENSOR_API_OK;
}
