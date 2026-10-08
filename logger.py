import serial
import time

# Hardcoded 
PORT = 'COM3' 
BAUD_RATE = 115200

try:

    ser = serial.Serial(PORT, BAUD_RATE)
    print(f"Connectong to {PORT}. Reading data...")


    with open("log1.csv", "a", encoding="utf-8") as file:
        file.write("Time(s), Temperature(C)\n")
        
        while True:
            if ser.in_waiting > 0:

                line = ser.readline().decode('utf-8').strip()
                print(f"got: {line}")
                

                file.write(f"{line}\n")
                file.flush()

except KeyboardInterrupt:
    print("\nWriting was interrupted")
except Exception as e:
    print(f"Error: {e}")
