#include <stdio.h>
#include <stdint.h>
#include <unistd.h>
#include <fcntl.h>
#include <sys/ioctl.h>
#include <linux/i2c-dev.h>
#include <linux/i2c.h>

// BMP388 I2C address
#define BMP388_I2C_ADDR 0x76

// BMP388 register addresses
#define BMP388_CHIP_ID_REG 0x00
#define BMP388_PRESS_MSB_REG 0x04
#define BMP388_TEMP_MSB_REG 0x07
#define BMP388_CONFIG_REG 0x1F
#define BMP388_RESET_REG 0x7E
#define BMP388_RESET_CMD 0xB6

static int g_dev_fd;

// Function to write to a register
int
bmp388_write_register(uint8_t reg, uint8_t value)
{
    uint8_t buffer[2] = {reg, value};
    if (write(g_dev_fd, buffer, 2) != 2) {
        perror("Failed to write to I2C register");
        return -1;
    }
    return 0;
}

// Function to read from a register
int
bmp388_read_register(uint8_t reg, uint8_t *data, size_t length)
{
    struct i2c_rdwr_ioctl_data packets;
    struct i2c_msg messages[2];

    messages[0].addr = BMP388_I2C_ADDR;
    messages[0].flags = 0;  // Write mode
    messages[0].len = 1;
    messages[0].buf = &reg;

    messages[1].addr = BMP388_I2C_ADDR;
    messages[1].flags = I2C_M_RD;  // Read mode
    messages[1].len = length;
    messages[1].buf = data;

    packets.msgs = messages;
    packets.nmsgs = 2;

    if (ioctl(g_dev_fd, I2C_RDWR, &packets) < 0) {
        perror("Failed to read I2C register");
        return -1;
    }
    return 0;
}

// Function to initialize BMP388
int
bmp388_init()
{
    // Open the I2C device
    const char *device = "/dev/i2c-1";
    if ((g_dev_fd = open(device, O_RDWR)) < 0) {
        perror("Failed to open the i2c bus");
        return -1;
    }

    // Specify the address of the BMP388
    if (ioctl(g_dev_fd, I2C_SLAVE, BMP388_I2C_ADDR) < 0) {
        perror("Failed to acquire bus access and/or talk to slave");
        return -1;
    }
#if 0
    // Check if the device is BMP388 by reading the chip ID
    uint8_t chip_id;
    if (bmp388_read_register(BMP388_CHIP_ID_REG, (uint8_t *)&chip_id, 1) < 0) {
        return -1;
    }

    if (chip_id != 0x50) {  // BMP388 chip ID should be 0x50
        fprintf(stderr, "Device is not a BMP388. Chip ID: 0x%x\n", chip_id);
        return -1;
    }

    // Set up configuration registers (e.g., oversampling, filter)
    // This is an example; customize it for your configuration.
    if (bmp388_write_register(BMP388_CONFIG_REG, 0x33) < 0) {
        return -1;
    }
#endif
    return 0;
}

// Function to write a reset command to BMP388
int
bmp388_reset()
{
    uint8_t buffer[2] = {BMP388_RESET_REG, BMP388_RESET_CMD};
    
    struct i2c_rdwr_ioctl_data packets;
    struct i2c_msg messages[1];

    // Set up the message for writing reset command
    messages[0].addr = BMP388_I2C_ADDR;
    messages[0].flags = 0; // Write mode
    messages[0].len = 2;   // Length of data (register + value)
    messages[0].buf = buffer;

    packets.msgs = messages;
    packets.nmsgs = 1;

    if (ioctl(g_dev_fd, I2C_RDWR, &packets) < 0) {
        perror("Failed to write reset command to BMP388");
        return -1;
    }
    return 0;
}

int
bmp388_deinit()
{
    close(g_dev_fd);
}

#if 0
// Function to read pressure from BMP388
float bmp388_read_pressure(int g_dev_fd) {
    uint8_t pressure_data[3];
    if (bmp388_read_register(g_dev_fd, BMP388_PRESS_MSB_REG, pressure_data, 3) < 0) {
        return -1;
    }

    // Convert 20-bit pressure data (BMP388 format)
    int32_t raw_pressure = ((int32_t)pressure_data[0] << 16) | 
                           ((int32_t)pressure_data[1] << 8) | 
                            (int32_t)pressure_data[2];
    raw_pressure >>= 4;  // Shift to get 20-bit value

    // Apply conversion factor to get pressure in Pascals (use BMP388 formula)
    float pressure = raw_pressure / 256.0;  // Replace with correct BMP388 scaling

    return pressure;
}

// Function to read temperature from BMP388
float bmp388_read_temperature(int g_dev_fd) {
    uint8_t temp_data[3];
    if (bmp388_read_register(g_dev_fd, BMP388_TEMP_MSB_REG, temp_data, 3) < 0) {
        return -1;
    }

    // Convert 20-bit temperature data (BMP388 format)
    int32_t raw_temp = ((int32_t)temp_data[0] << 16) |
                       ((int32_t)temp_data[1] << 8) |
                        (int32_t)temp_data[2];
    raw_temp >>= 4;  // Shift to get 20-bit value

    // Apply conversion factor to get temperature in Celsius (use BMP388 formula)
    // Assuming raw_temp scaling factor for temperature conversion
    float temperature = raw_temp / 512.0;  // Replace with correct BMP388 scaling

    return temperature;
}

int main() {
    int g_dev_fd;

    // Open the I2C device
    const char *device = "/dev/i2c-1";
    if ((g_dev_fd = open(device, O_RDWR)) < 0) {
        perror("Failed to open the i2c bus");
        return -1;
    }

    // Specify the address of the BMP388
    if (ioctl(g_dev_fd, I2C_SLAVE, BMP388_I2C_ADDR) < 0) {
        perror("Failed to acquire bus access and/or talk to slave");
        return -1;
    }

    // Initialize BMP388
    if (bmp388_init(g_dev_fd) < 0) {
        fprintf(stderr, "BMP388 initialization failed\n");
        return -1;
    }

    // Perform reset on BMP388
    if (bmp388_reset(g_dev_fd) < 0) {
        fprintf(stderr, "BMP388 reset failed\n");
        close(g_dev_fd);
        return -1;
    }

    printf("BMP388 reset successfully.\n");    // Read and print pressure

    float pressure = bmp388_read_pressure(g_dev_fd);
    printf("Pressure: %.2f Pa\n", pressure);

    // Read and print temperature
    float temperature = bmp388_read_temperature(g_dev_fd);
    printf("Temperature: %.2f °F\n", (temperature * 9/5) + 32);

    close(g_dev_fd);
    return 0;
}
#endif
