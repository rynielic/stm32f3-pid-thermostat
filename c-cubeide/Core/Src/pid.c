#include <stdint.h>
#include <stddef.h>

#include "pid.h"

static float float_clamp(float target, float low, float high){
	if (target < low) return low;
	if (target > high) return high;

	return target;
}

struct Pid pid_new(float kp, float ki, float kd, float setpoint) {
	struct Pid pid = {
		.kp = kp,
		.ki = ki,
		.kd = kd,
		.setpoint = setpoint,
		.previous_error = 0.0f,
		.integral = 0.0f,
	};
	return pid;
}

float pid_output(struct Pid *pid, float current_temperature, float dt) {
	if (dt < 0.000001f){ // protection from division by zero
		return 0.0f;
	}

	float temperature_error = pid->setpoint - current_temperature;

	// Bang-bang control
	float threshold;
	if (pid->setpoint > 65.0f) {
		threshold = 4.0f;
	} else if (pid->setpoint > 50.0f) {
		threshold = 5.0f;
	} else if (pid->setpoint > 35.0f){
		threshold = 7.0f;
	} else {
		threshold = 999.0f;
	}
	if (temperature_error > threshold) {
		return 100.0f;
	}

	// Integral term
	pid->integral += float_clamp(temperature_error * dt, -50.0f, 50.0f);
	// Derivative term
	float derivative = (temperature_error - pid->previous_error) / dt;

	// Output power
	float output = (
		pid->kp * temperature_error +
		pid->ki * pid->integral +
		pid->kd * derivative
	);
	output = float_clamp(output, 0.0f, 100.0f);

	// Update previous error
	pid->previous_error = temperature_error;

	return output;
}


