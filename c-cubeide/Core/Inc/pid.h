/*
 * pid.h
 *
 *  Created on: Nov 10, 2026
 *      Author: daniil
 */

#ifndef INC_PID_H_
#define INC_PID_H_

struct Pid {
	float kp;
	float ki;
	float kd;
	float setpoint;
	float previous_error;
	float integral;
};

struct Pid pid_new(float kp, float ki, float kd, float setpoint);

float pid_output(struct Pid *pid, float current_temperature, float dt);

#endif /* INC_PID_H_ */
