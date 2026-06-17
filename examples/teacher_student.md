# Teacher-Student Distillation With Latent Residuals

The method uses a large teacher language model to provide latent residual supervision for a compact student model. The student receives the task input, predicts the final answer, and is trained with both task loss and a dashed latent residual signal from the teacher. At inference time only the student model is used.
