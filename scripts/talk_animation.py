import bpy
import math

bpy.ops.wm.read_factory_settings(use_empty=True)
bpy.ops.import_scene.gltf(filepath="D:/Descargas/neeko.glb")

armature = None
for obj in bpy.data.objects:
    if obj.type == 'ARMATURE':
        armature = obj
        break

if not armature:
    print("ERROR: No armature found")
    exit()

bpy.context.scene.render.fps = 30
total_frames = 60

talk_action = bpy.data.actions.new(name="Talk_Loop")
armature.animation_data_create()
armature.animation_data.action = talk_action

def animate_bone(bone_name, value_func_x=None, value_func_y=None, value_func_z=None):
    pb = armature.pose.bones.get(bone_name)
    if not pb:
        print(f"  Bone not found: {bone_name}")
        return
    pb.rotation_mode = 'XYZ'
    for i in range(total_frames):
        t = i / 30.0
        frame = i + 1
        if value_func_x:
            pb.rotation_euler.x = value_func_x(t)
        if value_func_y:
            pb.rotation_euler.y = value_func_y(t)
        if value_func_z:
            pb.rotation_euler.z = value_func_z(t)
        pb.keyframe_insert(data_path="rotation_euler", frame=frame)
    print(f"  {bone_name} OK")

print("Talk_Loop animation:")
animate_bone("Jaw", value_func_x=lambda t: max(0, math.sin(t * 8.0)) * 0.25 + max(0, math.sin(t * 13.0 + 0.7)) * 0.1)
animate_bone("Head", value_func_x=lambda t: math.sin(t * 4.0) * 0.06, value_func_y=lambda t: math.sin(t * 2.5) * 0.04)
animate_bone("Neck", value_func_x=lambda t: math.sin(t * 4.0) * 0.03)
animate_bone("L_MouthCrnr", value_func_y=lambda t: max(0, math.sin(t * 8.0)) * 0.08)
animate_bone("R_MouthCrnr", value_func_y=lambda t: max(0, math.sin(t * 8.0 + 0.3)) * -0.08)
animate_bone("C_Mouth_All", value_func_z=lambda t: math.sin(t * 6.0) * 0.03)
animate_bone("L_Brow", value_func_x=lambda t: math.sin(t * 3.0) * 0.05)
animate_bone("R_Brow", value_func_x=lambda t: math.sin(t * 3.0 + 0.5) * 0.05)

armature.animation_data.action = None

bpy.ops.export_scene.gltf(
    filepath="D:/NEEKO API/neeko-assistant/src/neeko.glb",
    export_format='GLB',
    export_animations=True,
    export_yup=True,
)

print("Exported with original scale")
