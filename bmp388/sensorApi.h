#ifndef SENSORAPI_H_
#define SENSORAPI_H_

#define BMP388_I2C_ADDR 0x76 // Use 0x77 if ADDR pin is pulled high
#define I2C_DEV_PATH "/dev/i2c-1"
#define ITERATION  UINT8_C(100)

typedef enum {
    SENSOR_API_OK,
    SENSOR_API_NULL_PARAM,
    SENSOR_API_NONE}
    sensor_return_t;

typedef struct
{
    // Compensated temperature & pressure
    double temperature;
    double pressure;
} sensor_data_t;

int getSensorApi(sensor_data_t *);

#endif /* SENSORAPI_H_ */
