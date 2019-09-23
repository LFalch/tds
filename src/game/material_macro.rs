macro_rules! mat {
    (
        MISSING = $missing:ident
        $($mat:ident = $id:expr, $spr:expr, $solid:expr,)+
    ) => (
        #[derive(Debug, Copy, Clone, PartialEq, Eq)]
        #[repr(u8)]
        pub enum Material {
            $(
                $mat = $id,
            )*
        }

        impl Serialize for Material {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
                where S: Serializer {
                serializer.serialize_u8(*self as u8)
            }
        }

        impl From<u8> for Material {
            fn from(n: u8) -> Self {
                match n {
                    $(
                        $id => Material::$mat,
                    )*
                    _ => Material::$missing,
                }
            }
        }

        impl<'de> Deserialize<'de> for Material {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
                where D: Deserializer<'de> {
                <u8>::deserialize(deserializer).map(Material::from)
            }
        }

        impl Material {
            pub fn get_spr(&self) -> &'static str {
                match *self {
                    $(
                        Material::$mat => $spr,
                    )*
                }
            }
            pub fn solid(&self) -> bool {
                match *self {
                    $(
                        Material::$mat => $solid,
                    )*
                }
            }
            pub fn draw(&self, ctx: &mut Context, assets: &Assets, x: f32, y: f32, dp: graphics::DrawParam) -> GameResult<()> {
                let img = assets.get_img(ctx, self.get_spr());
                graphics::draw(ctx, &*img, (Point2::from(dp.dest) + Vector2::new(x, y),))
            }
        }
    );
}
